//! Dependency-aware talent unlocking.
//!
//! The talent/blueprint trees are a DAG: a node's `required_talents` may
//! point at other storable nodes, at `default_unlocked` nodes the game
//! grants for free, or at invisible "connector"/reroute nodes that only
//! exist to draw the tree. Real save files never contain connector rows, so
//! the cascade treats non-storable nodes as transparent: it never writes
//! them, but recurses through their own requirements.
//!
//! Talents also grant `UnlockedFlags` entries through their per-rank
//! Rewards. Rather than tracking individual grant/revoke deltas, every
//! mutation ends with a full resync: flags that any talent *could* grant
//! are derived from the current talent list, while flags outside that
//! universe (quest/progression state) are left untouched.

use crate::data::{DEPENDENTS, TALENTS_BY_NAME};
use crate::talent_owner::{FlagOwner, TalentOwner};
use shared::ResolvedTalent;
use std::collections::HashSet;

/// Unlocks or locks `name`, cascading through the dependency graph, then
/// resyncs talent-granted flags.
pub fn set_talent_cascading<T: TalentOwner + FlagOwner>(owner: &mut T, name: &str, unlocked: bool) {
    let mut visited = HashSet::new();
    if unlocked {
        unlock_with_deps(owner, name, &mut visited);
    } else {
        owner.set_talent(name, false);
        remove_dependents(owner, name, &mut visited);
    }
    sync_granted_flags(owner);
}

/// Bulk-unlocks every listed talent at its maximum rank (plus
/// prerequisites, via the same cascade rules as a single unlock), with one
/// flag resync at the end. Prerequisites pulled in from outside the list
/// stay at rank 1.
pub fn unlock_all<T: TalentOwner + FlagOwner>(owner: &mut T, talents: &[&'static ResolvedTalent]) {
    let mut visited = HashSet::new();
    for t in talents {
        unlock_with_deps(owner, &t.name, &mut visited);
        let max_rank = t.rewards.len().max(1) as i64;
        if max_rank > 1 && owner.has_talent(&t.name) {
            owner.set_talent_rank(&t.name, max_rank);
        }
    }
    sync_granted_flags(owner);
}

/// Bulk-locks every listed talent and everything that depends on any of
/// them, with one flag resync at the end.
pub fn lock_all<T: TalentOwner + FlagOwner>(owner: &mut T, talents: &[&'static ResolvedTalent]) {
    for t in talents {
        owner.set_talent(&t.name, false);
    }
    let mut visited = HashSet::new();
    for t in talents {
        remove_dependents(owner, &t.name, &mut visited);
    }
    sync_granted_flags(owner);
}

/// Unlocks or locks `name` alone -- no dependency cascade -- then resyncs
/// flags. Used when the user explicitly chooses "only this one" in the
/// prerequisite confirmation.
pub fn set_talent_only<T: TalentOwner + FlagOwner>(owner: &mut T, name: &str, unlocked: bool) {
    owner.set_talent(name, unlocked);
    sync_granted_flags(owner);
}

/// Every storable prerequisite of `talent` (transitively, looking through
/// connector nodes) that is not yet unlocked. Empty means the talent can be
/// unlocked without touching anything else.
pub fn locked_prerequisites<T: TalentOwner>(owner: &T, talent: &ResolvedTalent) -> Vec<&'static ResolvedTalent> {
    fn walk<T: TalentOwner>(
        owner: &T,
        name: &str,
        visited: &mut HashSet<&'static str>,
        out: &mut Vec<&'static ResolvedTalent>,
    ) {
        let Some(t) = TALENTS_BY_NAME.get(name).copied() else { return };
        if !visited.insert(t.name.as_str()) {
            return;
        }
        if t.default_unlocked {
            return;
        }
        if T::storable(t.category) {
            if owner.has_talent(&t.name) {
                return;
            }
            out.push(t);
        }
        for r in &t.required_talents {
            walk(owner, r, visited, out);
        }
    }
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    for r in &talent.required_talents {
        walk(owner, r, &mut visited, &mut out);
    }
    out
}

/// Every currently-unlocked talent that (transitively, through connectors)
/// requires `name`. Empty means locking `name` strands nothing.
pub fn unlocked_dependents<T: TalentOwner>(owner: &T, name: &str) -> Vec<&'static ResolvedTalent> {
    fn walk<T: TalentOwner>(
        owner: &T,
        name: &str,
        visited: &mut HashSet<&'static str>,
        out: &mut Vec<&'static ResolvedTalent>,
    ) {
        let Some(deps) = DEPENDENTS.get(name) else { return };
        for dep in deps {
            if !visited.insert(dep) {
                continue;
            }
            let Some(t) = TALENTS_BY_NAME.get(dep).copied() else { continue };
            if T::storable(t.category) {
                if owner.has_talent(dep) {
                    out.push(t);
                    walk(owner, dep, visited, out);
                }
            } else {
                walk(owner, dep, visited, out);
            }
        }
    }
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    walk(owner, name, &mut visited, &mut out);
    out
}

/// Changes a talent's rank and resyncs flags (higher ranks can grant
/// additional flags through Rewards[rank-1]).
pub fn set_talent_rank_synced<T: TalentOwner + FlagOwner>(owner: &mut T, name: &str, rank: i64) {
    owner.set_talent_rank(name, rank);
    sync_granted_flags(owner);
}

fn unlock_with_deps<T: TalentOwner>(owner: &mut T, name: &str, visited: &mut HashSet<&'static str>) {
    let Some(t) = TALENTS_BY_NAME.get(name) else { return };
    if !visited.insert(t.name.as_str()) {
        return;
    }
    if t.default_unlocked {
        return;
    }
    if T::storable(t.category) {
        if owner.has_talent(&t.name) {
            return;
        }
        owner.set_talent(&t.name, true);
    }
    for req in &t.required_talents {
        unlock_with_deps(owner, req, visited);
    }
}

/// Removes every stored talent that (directly, or transitively through
/// connector nodes) requires `name`.
fn remove_dependents<T: TalentOwner>(owner: &mut T, name: &str, visited: &mut HashSet<&'static str>) {
    let Some(dependents) = DEPENDENTS.get(name) else { return };
    for dep in dependents {
        if !visited.insert(dep) {
            continue;
        }
        let Some(t) = TALENTS_BY_NAME.get(dep) else { continue };
        if T::storable(t.category) {
            if owner.has_talent(dep) {
                owner.set_talent(dep, false);
                remove_dependents(owner, dep, visited);
            }
        } else {
            // Transparent connector: anything hanging off it loses its
            // path to `name` as well.
            remove_dependents(owner, dep, visited);
        }
    }
}

/// Whether all of a talent's requirements are satisfied for `owner`:
/// a requirement counts as met if it's unlocked, granted by default, or is
/// a transparent (non-storable) node whose own requirements are met.
pub fn requirements_met<T: TalentOwner>(owner: &T, talent: &ResolvedTalent) -> bool {
    let mut visited = HashSet::new();
    talent.required_talents.iter().all(|r| dep_satisfied(owner, r, &mut visited))
}

fn dep_satisfied<T: TalentOwner>(owner: &T, name: &str, visited: &mut HashSet<&'static str>) -> bool {
    let Some(t) = TALENTS_BY_NAME.get(name) else {
        // Requirement row missing from the bundled data (a couple exist in
        // the game files); don't block on what we can't see.
        return true;
    };
    if !visited.insert(t.name.as_str()) {
        return true;
    }
    if t.default_unlocked {
        return true;
    }
    if T::storable(t.category) {
        return owner.has_talent(&t.name);
    }
    t.required_talents.iter().all(|r| dep_satisfied(owner, r, visited))
}

/// Recomputes the talent-granted portion of `UnlockedFlags` from the
/// current talent list. Flags no talent can grant are preserved as-is.
fn sync_granted_flags<T: TalentOwner + FlagOwner>(owner: &mut T) {
    // Which flags should be on: union over unlocked talents of the flags
    // granted by reward tiers up to the current rank.
    let mut desired: HashSet<i64> = HashSet::new();
    let unlocked: Vec<(String, i64)> = owner
        .talent_entries()
        .iter()
        .map(|e| (e.row_name.clone(), e.rank))
        .collect();
    for (row, rank) in &unlocked {
        let Some(t) = TALENTS_BY_NAME.get(row.as_str()) else { continue };
        let tiers = (*rank).clamp(1, t.rewards.len() as i64) as usize;
        for reward in t.rewards.iter().take(tiers) {
            for flag in &reward.granted_flags {
                if let Some(idx) = T::flag_index(flag) {
                    desired.insert(idx);
                }
            }
        }
    }
    // The universe of flags any talent could grant (in this save file's
    // flag table): these are fully derived state, everything else is
    // untouched progression/quest state.
    for t in TALENTS_BY_NAME.values() {
        for reward in &t.rewards {
            for flag in &reward.granted_flags {
                if let Some(idx) = T::flag_index(flag) {
                    owner.set_flag(idx, desired.contains(&idx));
                }
            }
        }
    }
}
