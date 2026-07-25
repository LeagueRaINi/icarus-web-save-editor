use shared::save::TalentEntry;
use shared::Category;

/// Anything that has an Icarus-style `Talents: [{RowName, Rank}]` list --
/// implemented by both `CharacterSave` (Characters.json) and `ProfileSave`
/// (Profile.json, for orbital workshop research unlocks). Lets the talent
/// browser / dependency logic be written once and reused for both save
/// files instead of duplicated.
pub trait TalentOwner {
    fn has_talent(&self, row_name: &str) -> bool;
    fn set_talent(&mut self, row_name: &str, unlocked: bool);
    fn talent_rank(&self, row_name: &str) -> Option<i64>;
    fn set_talent_rank(&mut self, row_name: &str, rank: i64);
    fn talent_entries(&self) -> &[TalentEntry];

    /// Whether this save file actually stores rows of this category in its
    /// Talents[] list (verified against real saves: characters store only
    /// Talent/Blueprint, profiles only Workshop; connectors and everything
    /// else are never written).
    fn storable(category: Category) -> bool;

    /// Index of a flag RowName in this save file's flag table
    /// (D_CharacterFlags for characters, D_AccountFlags for profiles).
    fn flag_index(name: &str) -> Option<i64>;
}

/// Anything with an integer-indexed `UnlockedFlags: [int]` array -- both
/// CharacterSave and ProfileSave have one (against different flag tables).
/// The editor manages these automatically as a side-effect of talent
/// unlocks; they are not user-editable.
pub trait FlagOwner {
    fn set_flag(&mut self, index: i64, on: bool);
}
