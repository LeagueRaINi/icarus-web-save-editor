pub mod json_fmt;
pub mod save;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Talent,
    Blueprint,
    Workshop,
    Mission,
    Creature,
    Settlement,
    Connector,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Talent => "Talent",
            Category::Blueprint => "Blueprint",
            Category::Workshop => "Orbital Workshop",
            Category::Mission => "Mission",
            Category::Creature => "Creature",
            Category::Settlement => "Settlement",
            Category::Connector => "Connector",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RequirementRef {
    pub row: String,
    pub display_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RewardInfo {
    pub granted_stats: Vec<(String, f64)>,
    pub granted_flags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedTalent {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub icon: Option<String>,
    /// Filename (under the web app's `assets/icons/` dir) of the PNG decoded
    /// from `icon`, if extraction/decoding succeeded. `icon` itself stays a
    /// raw `/Game/...` content path -- not directly usable by a browser.
    pub icon_file: Option<String>,
    pub tree: String,
    pub tree_display_name: String,
    pub archetype: String,
    pub archetype_display_name: String,
    pub category: Category,
    pub position: (i64, i64),
    pub size: (i64, i64),
    pub required_talents: Vec<String>,
    pub required_rank: Option<RequirementRef>,
    pub required_level: i64,
    pub required_flags: Vec<RequirementRef>,
    pub forbidden_flags: Vec<RequirementRef>,
    pub rewards: Vec<RewardInfo>,
    pub default_unlocked: bool,
    pub draw_method: String,
    pub source_table: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedTree {
    pub name: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub icon_file: Option<String>,
    pub archetype: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedRank {
    pub name: String,
    pub display_name: String,
    pub investment: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedFlag {
    pub index: usize,
    pub name: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedCurrency {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_file: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct OutputBundle {
    pub talents: Vec<ResolvedTalent>,
    pub trees: Vec<ResolvedTree>,
    pub ranks: Vec<ResolvedRank>,
    pub character_flags: Vec<ResolvedFlag>,
    pub account_flags: Vec<ResolvedFlag>,
    pub currencies: Vec<ResolvedCurrency>,
}
