//! The on-disk save-file model: Characters.json and Profile.json from
//! `%LOCALAPPDATA%/Icarus/Saved/PlayerData/<steam-id>/`.
//!
//! Anything the game writes that this model doesn't explicitly cover is
//! captured via `#[serde(flatten)]` and re-emitted untouched, and
//! serialization reproduces the game's own formatting byte-for-byte
//! (verified in `tests/roundtrip.rs` against real saves).

use crate::json_fmt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One entry of an Icarus-style `Talents: [{RowName, Rank}]` list, shared by
/// Characters.json and Profile.json.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TalentEntry {
    #[serde(rename = "RowName")]
    pub row_name: String,
    #[serde(rename = "Rank", default = "default_rank")]
    pub rank: i64,
}

fn default_rank() -> i64 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CharacterSave {
    #[serde(rename = "CharacterName")]
    pub character_name: String,
    #[serde(rename = "ChrSlot")]
    pub chr_slot: i64,
    #[serde(rename = "XP")]
    pub xp: i64,
    #[serde(rename = "XP_Debt")]
    pub xp_debt: i64,
    #[serde(rename = "IsDead")]
    pub is_dead: bool,
    #[serde(rename = "IsAbandoned")]
    pub is_abandoned: bool,
    // Absent in some saves; skip when None so we don't introduce `null`
    // keys the game's own writer would have omitted.
    #[serde(rename = "LastProspectId", default, skip_serializing_if = "Option::is_none")]
    pub last_prospect_id: Option<String>,
    #[serde(rename = "Location", default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(rename = "UnlockedFlags", default)]
    pub unlocked_flags: Vec<i64>,
    #[serde(rename = "MetaResources", default)]
    pub meta_resources: Vec<Value>,
    #[serde(rename = "Cosmetic", default)]
    pub cosmetic: Value,
    #[serde(rename = "Talents", default)]
    pub talents: Vec<TalentEntry>,
    #[serde(rename = "TimeLastPlayed", default)]
    pub time_last_played: i64,
    /// Anything the game writes that this editor doesn't explicitly model
    /// yet is preserved here and re-emitted untouched on save.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

#[derive(Serialize, Deserialize)]
struct CharactersFile {
    #[serde(rename = "Characters.json")]
    characters_json: Vec<String>,
}

pub fn parse_characters_file(raw: &str) -> Result<Vec<CharacterSave>, String> {
    let file: CharactersFile = serde_json::from_str(raw).map_err(|e| format!("outer wrapper: {e}"))?;
    file.characters_json
        .iter()
        .enumerate()
        .map(|(i, s)| serde_json::from_str(s).map_err(|e| format!("character #{i}: {e}")))
        .collect()
}

pub fn serialize_characters_file(characters: &[CharacterSave]) -> Result<String, String> {
    // Match the game's own formatting exactly: the inner escaped strings
    // are tab-indented with LF newlines, the outer wrapper is tab-indented
    // with CRLF.
    let strings: Result<Vec<String>, String> =
        characters.iter().map(json_fmt::to_string_tabbed).collect();
    let file = CharactersFile {
        characters_json: strings?,
    };
    json_fmt::to_string_tabbed_crlf(&file)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MetaResourceEntry {
    #[serde(rename = "MetaRow")]
    pub meta_row: String,
    #[serde(rename = "Count")]
    pub count: i64,
}

/// Profile.json holds account-wide state: currency balances, account-wide
/// flags, and orbital workshop research unlocks (same RowName+Rank
/// mechanism as a character's Talents[], just a different pool of rows).
/// It is a plain JSON object -- unlike Characters.json it is *not* wrapped
/// in an escaped-string array.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileSave {
    #[serde(rename = "UserID", default)]
    pub user_id: String,
    #[serde(rename = "MetaResources", default)]
    pub meta_resources: Vec<MetaResourceEntry>,
    #[serde(rename = "UnlockedFlags", default)]
    pub unlocked_flags: Vec<i64>,
    #[serde(rename = "Talents", default)]
    pub talents: Vec<TalentEntry>,
    #[serde(rename = "NextChrSlot", default)]
    pub next_chr_slot: i64,
    #[serde(rename = "DataVersion", default)]
    pub data_version: i64,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Highest currency amount the editor will write -- values beyond this
/// aren't reliably handled by the game.
pub const CURRENCY_MAX: i64 = 999_999;

impl ProfileSave {
    pub fn currency(&self, meta_row: &str) -> i64 {
        self.meta_resources.iter().find(|m| m.meta_row == meta_row).map(|m| m.count).unwrap_or(0)
    }

    pub fn set_currency(&mut self, meta_row: &str, count: i64) {
        let count = count.clamp(0, CURRENCY_MAX);
        if let Some(m) = self.meta_resources.iter_mut().find(|m| m.meta_row == meta_row) {
            m.count = count;
        } else {
            self.meta_resources.push(MetaResourceEntry {
                meta_row: meta_row.to_string(),
                count,
            });
        }
    }
}

pub fn parse_profile_file(raw: &str) -> Result<ProfileSave, String> {
    serde_json::from_str(raw).map_err(|e| e.to_string())
}

pub fn serialize_profile_file(profile: &ProfileSave) -> Result<String, String> {
    // Tab-indented + CRLF, exactly how the game writes Profile.json.
    json_fmt::to_string_tabbed_crlf(profile)
}
