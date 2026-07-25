use crate::data::{format_stat, prettify_row_name, tree_icon_file, BUNDLE, TALENTS_BY_NAME};
use crate::icon::Icon;
use crate::talent_owner::{FlagOwner, TalentOwner};
use crate::unlock::{
    lock_all, locked_prerequisites, requirements_met, set_talent_cascading, set_talent_only,
    set_talent_rank_synced, unlock_all, unlocked_dependents,
};
use leptos::ev::MouseEvent;
use leptos::prelude::*;
use shared::{Category, ResolvedTalent};
use std::collections::BTreeMap;

#[component]
pub fn TalentBrowser<T>(
    characters: RwSignal<Option<Vec<T>>>,
    selected: RwSignal<Option<usize>>,
    categories: &'static [Category],
) -> impl IntoView
where
    T: TalentOwner + FlagOwner + Clone + Send + Sync + 'static,
{
    // Default to the first real category (Talents for characters) rather
    // than the combined "All" view -- talents and blueprints are different
    // enough that mixing them by default is just noise.
    let category: RwSignal<Option<Category>> = RwSignal::new(categories.first().copied());
    let search: RwSignal<String> = RwSignal::new(String::new());

    let grouped = move || -> Vec<(String, String, Vec<&'static ResolvedTalent>)> {
        let cat = category.get();
        let q = search.get().to_lowercase();
        let mut groups: BTreeMap<String, (String, Vec<&'static ResolvedTalent>)> = BTreeMap::new();
        for t in BUNDLE.talents.iter() {
            if !T::storable(t.category) {
                continue;
            }
            if let Some(c) = cat {
                if t.category != c {
                    continue;
                }
            }
            if !q.is_empty()
                && !t.display_name.to_lowercase().contains(&q)
                && !t.description.to_lowercase().contains(&q)
            {
                continue;
            }
            let entry = groups.entry(t.tree.clone()).or_insert_with(|| {
                // Blueprint trees have no localized names -- the "display
                // name" is just the raw row name again; prettify those.
                let display = if t.tree_display_name.contains('_') {
                    prettify_row_name(&t.tree_display_name)
                } else {
                    t.tree_display_name.clone()
                };
                (display, Vec::new())
            });
            entry.1.push(t);
        }
        for (_, v) in groups.values_mut() {
            v.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        }
        groups.into_iter().map(|(tree, (display, items))| (tree, display, items)).collect()
    };

    view! {
        <div class="talent-browser">
            <div class="browser-controls">
                <div class="tabs">
                    <button
                        class:active=move || category.get().is_none()
                        on:click=move |_: MouseEvent| category.set(None)
                    >
                        "All"
                    </button>
                    {categories
                        .iter()
                        .map(|c| {
                            let c = *c;
                            view! {
                                <button
                                    class:active=move || category.get() == Some(c)
                                    on:click=move |_: MouseEvent| category.set(Some(c))
                                >
                                    {c.label()}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
                <input
                    class="search"
                    type="text"
                    placeholder="Search by name or description..."
                    prop:value=move || search.get()
                    on:input=move |ev| search.set(event_target_value(&ev))
                />
                <div class="bulk-actions">
                    <button
                        class="bulk-btn"
                        title="Unlock everything currently shown (category + search filter apply)"
                        on:click=move |_: MouseEvent| {
                            let items: Vec<&'static ResolvedTalent> =
                                grouped().into_iter().flat_map(|(_, _, v)| v).collect();
                            characters.update(|opt| {
                                if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                                    unlock_all(c, &items);
                                }
                            });
                        }
                    >
                        "Unlock all"
                    </button>
                    <button
                        class="bulk-btn"
                        title="Lock everything currently shown, plus anything that depends on it"
                        on:click=move |_: MouseEvent| {
                            let items: Vec<&'static ResolvedTalent> =
                                grouped().into_iter().flat_map(|(_, _, v)| v).collect();
                            characters.update(|opt| {
                                if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                                    lock_all(c, &items);
                                }
                            });
                        }
                    >
                        "Lock all"
                    </button>
                </div>
            </div>
            <p class="hint">
                "Click a card to unlock it, click again to raise its rank, and clicking at "
                "max rank locks it again. If a change would affect prerequisites or "
                "dependent unlocks, you'll be asked first. Cards with a "
                <span class="hint-warning">"yellow edge"</span>
                " are missing requirements."
            </p>
            <div class="tree-groups">
                <For
                    each=grouped
                    key=|(tree, _, _)| tree.clone()
                    children=move |(tree, display, items)| {
                        view! { <TreeGroup tree=tree display=display items=items characters=characters selected=selected /> }
                    }
                />
            </div>
        </div>
    }
}

/// A single collapsible tree-group. Rows are only mounted (and only start
/// reading/subscribing to `characters`) once the group is actually expanded
/// -- with ~2000 talents in the full data set, keeping every collapsed
/// group's rows live in the reactive graph at all times is what made every
/// checkbox toggle notify thousands of uninvolved closures and feel laggy.
#[component]
fn TreeGroup<T>(
    tree: String,
    display: String,
    items: Vec<&'static ResolvedTalent>,
    characters: RwSignal<Option<Vec<T>>>,
    selected: RwSignal<Option<usize>>,
) -> impl IntoView
where
    T: TalentOwner + FlagOwner + Clone + Send + Sync + 'static,
{
    let count = items.len();
    let is_open = RwSignal::new(count <= 12);
    let unlocked_count = {
        let items = items.clone();
        move || {
            characters.with(|opt| {
                let Some(list) = opt else { return 0 };
                let Some(idx) = selected.get() else { return 0 };
                let Some(c) = list.get(idx) else { return 0 };
                items.iter().filter(|t| c.has_talent(&t.name)).count()
            })
        }
    };
    let pct = {
        let unlocked_count = unlocked_count.clone();
        move || (unlocked_count() * 100).checked_div(count).unwrap_or(0)
    };
    let icon_file = tree_icon_file(&tree, &items).map(str::to_string);

    view! {
        <div class="tree-group" class:open=move || is_open.get()>
            <button class="tree-group-header" on:click=move |_| is_open.update(|o| *o = !*o)>
                <span class="disclosure">{move || if is_open.get() { "▾" } else { "▸" }}</span>
                <Icon file=icon_file size=24 class="tree-icon" />
                <span class="tree-group-title">{display}</span>
                <span class="tree-group-count">{move || unlocked_count()} " / " {count}</span>
                <span class="progress-track">
                    <span class="progress-fill" style=move || format!("width: {}%", pct())></span>
                </span>
            </button>
            {move || {
                is_open
                    .get()
                    .then(|| {
                        view! {
                            <div class="talent-cards">
                                {items
                                    .clone()
                                    .into_iter()
                                    .map(|t| view! { <TalentRow talent=t characters=characters selected=selected /> })
                                    .collect_view()}
                            </div>
                        }
                    })
            }}
        </div>
    }
}

/// Human-readable names for a talent's direct requirements. Transparent
/// connector/reroute rows have no display name of their own, so they are
/// expanded into *their* requirements instead (a couple of levels deep at
/// most in the real data).
fn requirement_names(talent: &ResolvedTalent) -> Vec<String> {
    fn push_names(row: &str, depth: usize, out: &mut Vec<String>) {
        if depth > 4 {
            return;
        }
        match TALENTS_BY_NAME.get(row) {
            Some(t) if !t.display_name.is_empty() => out.push(t.display_name.clone()),
            Some(t) if !t.required_talents.is_empty() => {
                for r in &t.required_talents {
                    push_names(r, depth + 1, out);
                }
            }
            _ => out.push(prettify_row_name(row)),
        }
    }
    let mut names = Vec::new();
    for r in &talent.required_talents {
        push_names(r, 0, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

#[component]
pub fn TalentRow<T>(
    talent: &'static ResolvedTalent,
    characters: RwSignal<Option<Vec<T>>>,
    selected: RwSignal<Option<usize>>,
) -> impl IntoView
where
    T: TalentOwner + FlagOwner + Clone + Send + Sync + 'static,
{
    let name = talent.name.clone();
    let max_rank = talent.rewards.len().max(1) as i64;

    // None = locked, Some(rank) = unlocked at that rank.
    let state = {
        let name = name.clone();
        move || -> Option<i64> {
            characters.with(|opt| {
                let list = opt.as_ref()?;
                let c = list.get(selected.get()?)?;
                c.has_talent(&name).then(|| c.talent_rank(&name).unwrap_or(1))
            })
        }
    };

    // Pending confirmation before a change that would touch other talents:
    // (true = unlocking with locked prerequisites, false = locking with
    // unlocked dependents) plus the display names involved.
    let pending: RwSignal<Option<(bool, Vec<String>)>> = RwSignal::new(None);

    // One click walks the cycle: locked -> rank 1 -> rank 2 -> ... -> max
    // rank -> locked again. Steps that would change *other* talents too
    // (missing prerequisites / stranded dependents) ask first.
    let on_click = {
        let name = name.clone();
        let state = state.clone();
        move |_: MouseEvent| {
            if pending.get_untracked().is_some() {
                pending.set(None);
                return;
            }
            let current = state();
            match current {
                Some(r) if r < max_rank => {
                    characters.update(|opt| {
                        if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                            set_talent_rank_synced(c, &name, r + 1);
                        }
                    });
                }
                None => {
                    let missing: Vec<String> = characters.with_untracked(|opt| {
                        opt.as_ref()
                            .zip(selected.get_untracked())
                            .and_then(|(l, i)| l.get(i))
                            .map(|c| {
                                locked_prerequisites(c, talent)
                                    .into_iter()
                                    .map(|t| t.display_name.clone())
                                    .collect()
                            })
                            .unwrap_or_default()
                    });
                    if missing.is_empty() {
                        characters.update(|opt| {
                            if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                                set_talent_only(c, &name, true);
                            }
                        });
                    } else {
                        pending.set(Some((true, missing)));
                    }
                }
                Some(_) => {
                    let stranded: Vec<String> = characters.with_untracked(|opt| {
                        opt.as_ref()
                            .zip(selected.get_untracked())
                            .and_then(|(l, i)| l.get(i))
                            .map(|c| {
                                unlocked_dependents(c, &name)
                                    .into_iter()
                                    .map(|t| t.display_name.clone())
                                    .collect()
                            })
                            .unwrap_or_default()
                    });
                    if stranded.is_empty() {
                        characters.update(|opt| {
                            if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                                set_talent_only(c, &name, false);
                            }
                        });
                    } else {
                        pending.set(Some((false, stranded)));
                    }
                }
            }
        }
    };

    let reqs_met = move || {
        characters.with(|opt| {
            let Some(list) = opt else { return true };
            let Some(idx) = selected.get() else { return true };
            let Some(c) = list.get(idx) else { return true };
            requirements_met(c, talent)
        })
    };

    // Stat effects at the rank currently shown: the actual rank when
    // unlocked, or a rank-1 preview while still locked (tiers are totals,
    // not increments, so one tier is the whole story).
    let effects = {
        let state = state.clone();
        move || -> Vec<String> {
            let tier = state().unwrap_or(1).clamp(1, max_rank) as usize;
            talent
                .rewards
                .get(tier - 1)
                .map(|r| {
                    r.granted_stats
                        .iter()
                        .map(|(stat, value)| format_stat(stat, *value))
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    let requirement_text = {
        let mut parts = Vec::new();
        if let Some(rank) = &talent.required_rank {
            parts.push(format!("Rank: {}", rank.display_name));
        }
        if talent.required_level > 0 {
            parts.push(format!("Level {}", talent.required_level));
        }
        let names = requirement_names(talent);
        if !names.is_empty() {
            parts.push(format!("Requires: {}", names.join(", ")));
        }
        parts.join("  ·  ")
    };

    let state_for_unlocked = state.clone();
    let state_for_badge = state.clone();
    let has_effects = talent.rewards.iter().any(|r| !r.granted_stats.is_empty());

    view! {
        <div
            class="talent-row"
            class:unlocked=move || state_for_unlocked().is_some()
            class:unmet=move || !reqs_met()
            on:click=on_click
        >
            <Icon file=talent.icon_file.clone() size=44 />
            <div class="talent-body">
                <span class="talent-name">{talent.display_name.clone()}</span>
                {(!talent.description.is_empty())
                    .then(|| view! { <span class="talent-desc">{talent.description.clone()}</span> })}
                {has_effects
                    .then(|| {
                        view! {
                            <span class="talent-effects">
                                {move || {
                                    effects()
                                        .into_iter()
                                        .map(|e| view! { <span class="effect-chip">{e}</span> })
                                        .collect_view()
                                }}
                            </span>
                        }
                    })}
                {(!requirement_text.is_empty())
                    .then(|| view! { <span class="talent-req">{requirement_text.clone()}</span> })}
                {{
                    let name_all = name.clone();
                    let name_only = name.clone();
                    move || {
                        pending.get().map(|(is_unlock, names)| {
                            let shown = if names.len() > 6 {
                                format!("{}, … and {} more", names[..6].join(", "), names.len() - 6)
                            } else {
                                names.join(", ")
                            };
                            let message = if is_unlock {
                                format!("This needs {} locked prerequisite(s): {shown}", names.len())
                            } else {
                                format!("{} unlocked item(s) depend on this: {shown}", names.len())
                            };
                            let name_all = name_all.clone();
                            let name_only = name_only.clone();
                            view! {
                                <div class="confirm-box" on:click=|ev: MouseEvent| ev.stop_propagation()>
                                    <span class="confirm-text">{message}</span>
                                    <div class="confirm-actions">
                                        <button
                                            class="confirm-primary"
                                            on:click=move |ev: MouseEvent| {
                                                ev.stop_propagation();
                                                pending.set(None);
                                                characters.update(|opt| {
                                                    if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                                                        set_talent_cascading(c, &name_all, is_unlock);
                                                    }
                                                });
                                            }
                                        >
                                            {if is_unlock { "Unlock all" } else { "Lock all" }}
                                        </button>
                                        <button
                                            on:click=move |ev: MouseEvent| {
                                                ev.stop_propagation();
                                                pending.set(None);
                                                characters.update(|opt| {
                                                    if let Some(c) = opt.as_mut().zip(selected.get()).and_then(|(l, i)| l.get_mut(i)) {
                                                        set_talent_only(c, &name_only, is_unlock);
                                                    }
                                                });
                                            }
                                        >
                                            "Only this one"
                                        </button>
                                        <button on:click=move |ev: MouseEvent| {
                                            ev.stop_propagation();
                                            pending.set(None);
                                        }>
                                            "Cancel"
                                        </button>
                                    </div>
                                </div>
                            }
                        })
                    }
                }}
            </div>
            {(max_rank > 1)
                .then(|| {
                    let state_for_text = state_for_badge.clone();
                    view! {
                        <span class="rank-badge" class=("rank-zero", move || state_for_badge().is_none())>
                            {move || format!("{} / {max_rank}", state_for_text().unwrap_or(0))}
                        </span>
                    }
                })}
        </div>
    }
}
