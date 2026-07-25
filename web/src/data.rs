use shared::{Category, OutputBundle, ResolvedFlag, ResolvedTalent, ResolvedTree};
use std::collections::HashMap;
use std::sync::LazyLock;

const BUNDLE_JSON: &str = include_str!("../assets/data/talents.json");

pub static BUNDLE: LazyLock<OutputBundle> =
    LazyLock::new(|| serde_json::from_str(BUNDLE_JSON).expect("bundled talents.json should parse"));

pub static TALENTS_BY_NAME: LazyLock<HashMap<&'static str, &'static ResolvedTalent>> =
    LazyLock::new(|| {
        BUNDLE
            .talents
            .iter()
            .map(|t| (t.name.as_str(), t))
            .collect()
    });

pub static TREES_BY_NAME: LazyLock<HashMap<&'static str, &'static ResolvedTree>> =
    LazyLock::new(|| BUNDLE.trees.iter().map(|t| (t.name.as_str(), t)).collect());

/// Reverse dependency index: talent RowName -> every talent that lists it in
/// `required_talents`. Used to cascade removals ("if you lock this, lock
/// everything that needed it too").
pub static DEPENDENTS: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    let mut map: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    for t in BUNDLE.talents.iter() {
        for req in &t.required_talents {
            map.entry(req.as_str()).or_default().push(t.name.as_str());
        }
    }
    map
});

fn flag_index_map(flags: &'static [ResolvedFlag]) -> HashMap<&'static str, i64> {
    flags.iter().map(|f| (f.name.as_str(), f.index as i64)).collect()
}

/// Flag RowName -> index in D_CharacterFlags (Characters.json UnlockedFlags).
pub static CHARACTER_FLAG_INDEX: LazyLock<HashMap<&'static str, i64>> =
    LazyLock::new(|| flag_index_map(&BUNDLE.character_flags));

/// Flag RowName -> index in D_AccountFlags (Profile.json UnlockedFlags).
pub static ACCOUNT_FLAG_INDEX: LazyLock<HashMap<&'static str, i64>> =
    LazyLock::new(|| flag_index_map(&BUNDLE.account_flags));

/// Icon files that many trees share are placeholder art in the game data
/// (every Workshop_* tree points at the same generic pickaxe icon) --
/// showing them tells the user nothing.
static SHARED_TREE_ICONS: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for t in BUNDLE.trees.iter() {
        if let Some(f) = t.icon_file.as_deref() {
            *counts.entry(f).or_default() += 1;
        }
    }
    counts.into_iter().filter(|(_, n)| *n > 2).map(|(f, _)| f).collect()
});

/// The tree's own icon, unless it's shared placeholder art -- then fall
/// back to the first item of the group so each header still gets a
/// distinctive image.
pub fn tree_icon_file(tree_row_name: &str, items: &[&'static ResolvedTalent]) -> Option<&'static str> {
    TREES_BY_NAME
        .get(tree_row_name)
        .and_then(|t| t.icon_file.as_deref())
        .filter(|f| !SHARED_TREE_ICONS.contains(f))
        .or_else(|| items.iter().find_map(|t| t.icon_file.as_deref()))
}

/// Category tabs shown per editor. Which categories a save file actually
/// stores lives on `TalentOwner::storable`.
pub const CATEGORIES: [Category; 2] = [Category::Talent, Category::Blueprint];
pub const WORKSHOP_CATEGORIES: [Category; 1] = [Category::Workshop];

/// Some rows have no localized display text at all (blueprint tree names
/// like `Blueprint_T1_Player`, connector rows, ...). Strip engine-y
/// prefixes and split words so the raw RowName at least reads like a label
/// (`Blueprint_T1_Player` -> `Tier 1 Player`).
pub fn prettify_row_name(name: &str) -> String {
    const PREFIXES: &[&str] = &[
        "GrantedBlueprint_",
        "GrantedWorkshop_",
        "GrantedTalent_",
        "Blueprint_",
        "Talent_",
        "Mission_",
        "NPC_",
    ];
    let mut rest = name;
    for prefix in PREFIXES {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped;
            break;
        }
    }
    rest = rest.trim_end_matches("_TooltipOnly");

    let mut out = String::with_capacity(rest.len() + 4);
    let mut prev: Option<char> = None;
    for c in rest.chars() {
        if c == '_' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            prev = None;
            continue;
        }
        if let Some(p) = prev {
            let boundary = (p.is_lowercase() || p.is_ascii_digit()) && c.is_uppercase();
            if boundary && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        out.push(c);
        prev = Some(c);
    }
    // "T1" / "T2" tier shorthand reads better spelled out.
    out.replace("T1", "Tier 1")
        .replace("T2", "Tier 2")
        .replace("T3", "Tier 3")
        .replace("T4", "Tier 4")
        .replace("T5", "Tier 5")
}

/// Formats a granted-stat row like `("BaseBowReloadSpeed_+%", 5.0)` into a
/// human-readable effect like `+5% Bow Reload Speed`.
pub fn format_stat(name: &str, value: f64) -> String {
    let (base, percent) = if let Some(b) = name.strip_suffix("_+%") {
        (b, true)
    } else if let Some(b) = name.strip_suffix("_%") {
        (b, true)
    } else {
        (name, false)
    };
    let base = base.strip_prefix("Base").unwrap_or(base);
    let label = prettify_row_name(base);

    // Trim "15.0" down to "15" but keep real fractions like "2.5".
    let num = if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    };
    let sign = if value >= 0.0 { "+" } else { "" };
    if percent {
        format!("{sign}{num}% {label}")
    } else {
        format!("{sign}{num} {label}")
    }
}
