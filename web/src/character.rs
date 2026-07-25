//! UI-side behavior for Characters.json saves. The data model and
//! serialization live in `shared::save`.

use crate::data::CHARACTER_FLAG_INDEX;
use crate::talent_owner::{FlagOwner, TalentOwner};
use shared::save::TalentEntry;
use shared::Category;

pub use shared::save::{parse_characters_file, serialize_characters_file, CharacterSave};

impl TalentOwner for CharacterSave {
    fn has_talent(&self, row_name: &str) -> bool {
        self.talents.iter().any(|t| t.row_name == row_name)
    }

    fn set_talent(&mut self, row_name: &str, unlocked: bool) {
        if unlocked {
            if !self.has_talent(row_name) {
                self.talents.push(TalentEntry {
                    row_name: row_name.to_string(),
                    rank: 1,
                });
            }
        } else {
            self.talents.retain(|t| t.row_name != row_name);
        }
    }

    fn talent_rank(&self, row_name: &str) -> Option<i64> {
        self.talents.iter().find(|t| t.row_name == row_name).map(|t| t.rank)
    }

    fn set_talent_rank(&mut self, row_name: &str, rank: i64) {
        if let Some(t) = self.talents.iter_mut().find(|t| t.row_name == row_name) {
            t.rank = rank;
        }
    }

    fn talent_entries(&self) -> &[TalentEntry] {
        &self.talents
    }

    fn storable(category: Category) -> bool {
        matches!(category, Category::Talent | Category::Blueprint)
    }

    fn flag_index(name: &str) -> Option<i64> {
        CHARACTER_FLAG_INDEX.get(name).copied()
    }
}

impl FlagOwner for CharacterSave {
    fn set_flag(&mut self, index: i64, on: bool) {
        if on {
            if !self.unlocked_flags.contains(&index) {
                self.unlocked_flags.push(index);
            }
        } else {
            self.unlocked_flags.retain(|f| *f != index);
        }
    }
}
