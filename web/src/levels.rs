//! Player level & progression-point estimation from total XP.
//!
//! XP anchors from the community table: exact totals for levels 0-20, then
//! every 5 levels to 100. The XP cost per level is constant inside each
//! bracket (verified against the table's "next level" column), so the
//! computed level is exact, not an estimate.
//!
//! Point totals follow an exact parity rule that reproduces every table
//! row: talents give +1 on odd levels / +2 on even up to level 60 (90
//! total); blueprints +4 on odd / +3 on even up to level 100 (350 total).
//!
//! Levelling is not the only talent-point source: high-tier mission
//! rewards grant account-wide bonus talent points (+2 NIGHTFALL, +2
//! IRONCLAD, +3 Null Sector, ...). Blueprint points come from levelling
//! alone, so those can be checked strictly.

use crate::data::TALENTS_BY_NAME;
use shared::save::TalentEntry;
use shared::Category;

/// (level, total XP required)
const XP_ANCHORS: &[(i64, i64)] = &[
    (0, 0),
    (1, 2_400),
    (2, 8_610),
    (3, 18_730),
    (4, 32_530),
    (5, 48_630),
    (6, 67_830),
    (7, 89_430),
    (8, 111_030),
    (9, 135_030),
    (10, 161_500),
    (11, 194_400),
    (12, 227_300),
    (13, 260_200),
    (14, 293_100),
    (15, 326_000),
    (16, 380_800),
    (17, 435_600),
    (18, 490_400),
    (19, 545_200),
    (20, 600_000),
    (25, 975_000),
    (30, 1_400_000),
    (35, 1_942_000),
    (40, 2_550_000),
    (45, 3_200_000),
    (50, 3_890_000),
    (55, 4_610_000),
    (60, 5_330_000),
    (65, 6_050_000),
    (70, 6_770_000),
    (75, 7_490_000),
    (80, 8_210_000),
    (85, 8_930_000),
    (90, 9_650_000),
    (95, 10_370_000),
    (100, 11_090_000),
];

/// Account-wide bonus talent points earnable from mission rewards, on top
/// of the level-based total. ~13 as of the 2023 mission set and more have
/// been added since; a legit level-30 character was observed 15 over the
/// level table. Kept generous so legitimate characters aren't flagged.
pub const MISSION_TALENT_BONUS_MAX: i64 = 20;

pub struct LevelEstimate {
    pub level: i64,
    /// Talent points earnable from levelling alone at this level.
    pub talent_points: i64,
    /// Blueprint points earnable at this level (levelling is the only
    /// source for these).
    pub blueprint_points: i64,
    /// Solo-pool talent points earnable at this level: one every 2 levels,
    /// capped at 30 (level 60).
    pub solo_points: i64,
}

fn talent_points_at(level: i64) -> i64 {
    let n = level.clamp(0, 60);
    (n / 2) * 3 + (n % 2)
}

fn blueprint_points_at(level: i64) -> i64 {
    let n = level.clamp(0, 100);
    (n / 2) * 7 + (n % 2) * 4
}

fn solo_points_at(level: i64) -> i64 {
    (level.max(0) / 2).min(30)
}

pub fn estimate(xp: i64) -> LevelEstimate {
    let xp = xp.max(0);
    let last = XP_ANCHORS.last().unwrap();
    let level = if xp >= last.1 {
        last.0
    } else {
        // Per-level cost is constant within a bracket, so integer
        // interpolation lands on the exact level.
        let idx = XP_ANCHORS.partition_point(|(_, total)| *total <= xp) - 1;
        let (al, ax) = XP_ANCHORS[idx];
        let (bl, bx) = XP_ANCHORS[idx + 1];
        al + (xp - ax) * (bl - al) / (bx - ax)
    };
    LevelEstimate {
        level,
        talent_points: talent_points_at(level),
        blueprint_points: blueprint_points_at(level),
        solo_points: solo_points_at(level),
    }
}

pub struct SpentPoints {
    /// Regular talent points (one per rank), excluding the Solo tree.
    pub talent: i64,
    /// Solo-tree talent points -- a separate pool with its own source,
    /// not counted against the level-based total. (Verified against a
    /// legitimate boosted character: regular trees matched the level
    /// table exactly once Solo was split out.)
    pub solo: i64,
    /// Blueprint points, one per unlocked blueprint.
    pub blueprint: i64,
}

/// Points actually spent by a character: talents cost one point per rank,
/// blueprints one point each. Rows the bundled data doesn't know are
/// ignored.
pub fn spent_points(talents: &[TalentEntry]) -> SpentPoints {
    let mut spent = SpentPoints {
        talent: 0,
        solo: 0,
        blueprint: 0,
    };
    for e in talents {
        let Some(t) = TALENTS_BY_NAME.get(e.row_name.as_str()) else { continue };
        match t.category {
            Category::Talent if t.archetype == "Solo" => spent.solo += e.rank.max(1),
            Category::Talent => spent.talent += e.rank.max(1),
            Category::Blueprint => spent.blueprint += 1,
            _ => {}
        }
    }
    spent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_totals_match_community_table() {
        // Spot checks straight from the table.
        for (level, talents, blueprints) in [
            (0, 0, 0),
            (1, 1, 4),
            (2, 3, 7),
            (10, 15, 35),
            (20, 30, 70),
            (25, 37, 88),
            (30, 45, 105),
            (45, 67, 158),
            (60, 90, 210),
            (65, 90, 228),
            (100, 90, 350),
        ] {
            assert_eq!(talent_points_at(level), talents, "talent points at {level}");
            assert_eq!(blueprint_points_at(level), blueprints, "blueprint points at {level}");
        }
        // Solo pool: one point every 2 levels, capped at 30.
        for (level, solo) in [(0, 0), (1, 0), (2, 1), (30, 15), (59, 29), (60, 30), (100, 30)] {
            assert_eq!(solo_points_at(level), solo, "solo points at {level}");
        }
    }

    #[test]
    fn level_from_xp_matches_table() {
        for (level, xp) in XP_ANCHORS {
            assert_eq!(estimate(*xp).level, *level, "level at exactly {xp} XP");
            if *level > 0 {
                assert_eq!(estimate(*xp - 1).level, *level - 1, "level just below {xp} XP");
            }
        }
        // Mid-bracket: 1,416,460 XP is level 30 (next level at 1,508,400).
        assert_eq!(estimate(1_416_460).level, 30);
        assert_eq!(estimate(20_000_000).level, 100);
    }
}
