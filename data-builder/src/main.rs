mod json;
mod pak_source;
mod texture;
mod uasset;

use anyhow::{bail, Result};
use clap::Parser;
use json::{bool_field, handle_row_name, i64_field, nsloctext, opt_str, text_field, DataTable};
use pak_source::{game_path_to_asset_base, PakSource};
use regex::Regex;
use shared::{
    Category, OutputBundle, RequirementRef, ResolvedCurrency, ResolvedFlag, ResolvedRank,
    ResolvedTalent, ResolvedTree, RewardInfo,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Icarus's standard Steam install location, used when no game root is given.
const DEFAULT_GAME_ROOT: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Icarus";

/// The repo's `web/assets/data` folder. Anchored to this crate's source
/// location (baked in at compile time by cargo) rather than the process's
/// current directory, so the output lands in the right place no matter which
/// directory `cargo run -p data-builder` was invoked from. The `..` is
/// resolved lexically where it is used.
const DEFAULT_OUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../web/assets/data");

/// Extracts talent/blueprint definitions and icons from the ICARUS pak files
/// into the JSON bundle and PNGs the web app consumes.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// ICARUS install directory (the folder containing `Icarus/Content`)
    #[arg(value_name = "GAME_ROOT", default_value = DEFAULT_GAME_ROOT)]
    game_root: PathBuf,

    /// Directory to write `talents.json` and `icons/` into
    #[arg(short, long, value_name = "DIR", default_value_os_t = path_clean::clean(DEFAULT_OUT_DIR))]
    out_dir: PathBuf,
}

/// Plain display text + optional icon resolved from whichever table actually
/// owns it for a given talent row.
#[derive(Default, Clone)]
struct DisplayInfo {
    name: String,
    description: String,
    icon: Option<String>,
}

struct Tables {
    talents: DataTable,
    trees: DataTable,
    archetypes: DataTable,
    ranks: DataTable,
    itemable: DataTable,
    workshop_items: DataTable,
    item_template: DataTable,
    items_static: DataTable,
    outposts: DataTable,
    prospect_list: DataTable,
    great_hunts: DataTable,
    settlement_buildings: DataTable,
    character_flags: DataTable,
    session_flags: DataTable,
    account_flags: DataTable,
    currencies: DataTable,
}

fn main() -> Result<()> {
    let Args { game_root, out_dir } = Args::parse();

    let content = game_root.join("Icarus/Content");
    if !content.is_dir() {
        bail!(
            "{} does not look like an ICARUS install: {} not found\n\
             pass the install directory as an argument, e.g. `-- \"{DEFAULT_GAME_ROOT}\"`",
            game_root.display(),
            content.display(),
        );
    }

    eprintln!("game root: {}", game_root.display());
    eprintln!("output:    {}", out_dir.display());

    let data_pak = PakSource::open(&[content.join("Data/data.pak")])?;

    let tables = Tables {
        talents: DataTable::load(&data_pak, "Talents/D_Talents.json")?,
        trees: DataTable::load(&data_pak, "Talents/D_TalentTrees.json")?,
        archetypes: DataTable::load(&data_pak, "Talents/D_TalentArchetypes.json")?,
        ranks: DataTable::load(&data_pak, "Talents/D_TalentRanks.json")?,
        itemable: DataTable::load(&data_pak, "Traits/D_Itemable.json")?,
        workshop_items: DataTable::load(&data_pak, "MetaWorkshop/D_WorkshopItems.json")?,
        item_template: DataTable::load(&data_pak, "Items/D_ItemTemplate.json")?,
        items_static: DataTable::load(&data_pak, "Items/D_ItemsStatic.json")?,
        outposts: DataTable::load(&data_pak, "Outpost/D_Outposts.json")?,
        prospect_list: DataTable::load(&data_pak, "Prospects/D_ProspectList.json")?,
        great_hunts: DataTable::load(&data_pak, "GreatHunt/D_GreatHunts.json")?,
        settlement_buildings: DataTable::load(&data_pak, "Settlement/D_SettlementBuildings.json")?,
        character_flags: DataTable::load(&data_pak, "Flags/D_CharacterFlags.json")?,
        session_flags: DataTable::load(&data_pak, "Flags/D_SessionFlags.json")?,
        account_flags: DataTable::load(&data_pak, "Flags/D_AccountFlags.json")?,
        currencies: DataTable::load(&data_pak, "Currency/D_MetaCurrency.json")?,
    };

    let mut talents = resolve_all_talents(&tables);
    let mut trees = resolve_trees(&tables);
    let ranks = resolve_ranks(&tables);
    let character_flags = resolve_character_flags(&tables);
    let account_flags = resolve_account_flags(&tables);
    let mut currencies = resolve_currencies(&tables);

    eprintln!(
        "resolved {} talents, {} trees, {} ranks, {} character flags, {} account flags, {} currencies",
        talents.len(),
        trees.len(),
        ranks.len(),
        character_flags.len(),
        account_flags.len(),
        currencies.len(),
    );
    let by_cat = |c: Category| talents.iter().filter(|t| t.category == c).count();
    eprintln!(
        "  Talent={} Blueprint={} Workshop={} Mission={} Creature={} Settlement={} Connector={}",
        by_cat(Category::Talent),
        by_cat(Category::Blueprint),
        by_cat(Category::Workshop),
        by_cat(Category::Mission),
        by_cat(Category::Creature),
        by_cat(Category::Settlement),
        by_cat(Category::Connector),
    );

    let icon_paks = PakSource::open(&[
        content.join("Paks/pakchunk0_s19-WindowsNoEditor.pak"),
        content.join("Paks/pakchunk0_s20-WindowsNoEditor.pak"),
    ])?;
    let icon_out_dir = out_dir.join("icons");
    std::fs::create_dir_all(&icon_out_dir)?;

    // Talent icons: only Talent/Blueprint/Workshop are ever shown in the UI
    // (Mission/Creature/Settlement include multi-MB background art nothing
    // displays). Tree and currency icons are a much smaller, fixed set, so
    // no filtering is needed there.
    let talent_icon_paths = talents
        .iter()
        .filter(|t| matches!(t.category, Category::Talent | Category::Blueprint | Category::Workshop))
        .filter_map(|t| t.icon.as_deref());
    let tree_icon_paths = trees.iter().filter_map(|t| t.icon.as_deref());
    let currency_icon_paths = currencies.iter().filter_map(|c| c.icon.as_deref());

    let unique_paths: std::collections::BTreeSet<&str> =
        talent_icon_paths.chain(tree_icon_paths).chain(currency_icon_paths).collect();
    let icon_map = extract_icons(&icon_paks, unique_paths, &icon_out_dir);

    for t in &mut talents {
        if let Some(game_path) = &t.icon {
            t.icon_file = icon_map.get(game_path).cloned();
        }
    }
    for tree in &mut trees {
        if let Some(game_path) = &tree.icon {
            tree.icon_file = icon_map.get(game_path).cloned();
        }
    }
    for c in &mut currencies {
        if let Some(game_path) = &c.icon {
            c.icon_file = icon_map.get(game_path).cloned();
        }
    }

    let bundle = OutputBundle {
        talents,
        trees,
        ranks,
        character_flags,
        account_flags,
        currencies,
    };

    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("talents.json");
    std::fs::write(&out_path, serde_json::to_vec(&bundle)?)?;
    eprintln!("wrote {}", out_path.display());

    Ok(())
}

fn resolve_all_talents(t: &Tables) -> Vec<ResolvedTalent> {
    t.talents
        .rows
        .iter()
        .filter_map(|raw_row| {
            let name = raw_row.get("Name")?.as_str()?.to_string();
            let row = t.talents.find(&name)?; // merged over Defaults
            Some(resolve_talent(t, &name, &row))
        })
        .collect()
}

fn resolve_talent(t: &Tables, name: &str, row: &Value) -> ResolvedTalent {
    let own_display_name = text_field(row, "DisplayName");
    let own_description = text_field(row, "Description");
    let own_icon = opt_str(row, "Icon");

    let extra_row_name = handle_row_name(row, "ExtraData");
    let extra_table = row
        .get("ExtraData")
        .and_then(|e| e.get("DataTableName"))
        .and_then(|v| v.as_str())
        .unwrap_or("None");

    let (linked, source_table) = match (extra_row_name.as_deref(), extra_table) {
        (Some(rn), "D_Itemable") => (resolve_itemable(t, rn), "D_Itemable"),
        (Some(rn), "D_WorkshopItems") => (resolve_workshop_item(t, rn), "D_WorkshopItems"),
        (Some(rn), "D_Outposts") => (resolve_outpost(t, rn), "D_Outposts"),
        (Some(rn), "D_ProspectList") => (resolve_prospect(t, rn), "D_ProspectList"),
        (Some(rn), "D_GreatHunts") => (resolve_great_hunt(t, rn), "D_GreatHunts"),
        (Some(rn), "D_SettlementBuildings") => {
            (resolve_settlement_building(t, rn), "D_SettlementBuildings")
        }
        _ => (None, "D_Talents"),
    };

    let display_name = non_empty(own_display_name).or_else(|| linked.as_ref().map(|l| l.name.clone())).unwrap_or_default();
    let description = non_empty(own_description).or_else(|| linked.as_ref().map(|l| l.description.clone())).unwrap_or_default();
    let icon = own_icon.or_else(|| linked.as_ref().and_then(|l| l.icon.clone()));

    let has_display_data = !display_name.is_empty() || icon.is_some();

    let tree_row_name = handle_row_name(row, "TalentTree").unwrap_or_default();
    let (tree_display_name, archetype_row_name, archetype_display_name, tree_icon) =
        resolve_tree(t, &tree_row_name);

    // "Settlement_Hub" isn't registered in D_TalentTrees.json at all, so
    // archetype-based classification can't see it -- and a few of its rows
    // (radius/wall upgrades) have no ExtraData link either, so per-node
    // source_table detection misses them too. Classify by tree membership
    // instead so the whole tree is consistently excluded from character
    // editing, not just the rows that happen to route through
    // D_SettlementBuildings.
    let category = if tree_row_name == "Settlement_Hub" || source_table == "D_SettlementBuildings" {
        Category::Settlement
    } else {
        classify(&archetype_row_name, has_display_data)
    };
    let _ = tree_icon; // trees have their own icon (used for category/tree headers), not a per-talent fallback

    let required_talents = row
        .get("RequiredTalents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|h| h.get("RowName").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let required_rank = handle_row_name(row, "RequiredRank").map(|rn| RequirementRef {
        display_name: t
            .ranks
            .find(&rn)
            .map(|r| text_field(&r, "DisplayName"))
            .unwrap_or_else(|| rn.clone()),
        row: rn,
    });

    let required_flags = resolve_flag_refs(t, row, "RequiredFlags");
    let forbidden_flags = resolve_flag_refs(t, row, "ForbiddenFlags");
    let rewards = resolve_rewards(row);

    let position = (
        row.get("Position").and_then(|p| p.get("X")).and_then(|v| v.as_i64()).unwrap_or(0),
        row.get("Position").and_then(|p| p.get("Y")).and_then(|v| v.as_i64()).unwrap_or(0),
    );
    let size = (
        row.get("Size").and_then(|p| p.get("X")).and_then(|v| v.as_i64()).unwrap_or(64),
        row.get("Size").and_then(|p| p.get("Y")).and_then(|v| v.as_i64()).unwrap_or(64),
    );

    ResolvedTalent {
        name: name.to_string(),
        display_name,
        description,
        icon,
        icon_file: None, // filled in by extract_icons() after all talents are resolved
        tree: tree_row_name,
        tree_display_name,
        archetype: archetype_row_name,
        archetype_display_name,
        category,
        position,
        size,
        required_talents,
        required_rank,
        required_level: i64_field(row, "RequiredLevel"),
        required_flags,
        forbidden_flags,
        rewards,
        default_unlocked: bool_field(row, "bDefaultUnlocked"),
        draw_method: row
            .get("DrawMethodOverride")
            .and_then(|v| v.as_str())
            .unwrap_or("Unspecified")
            .to_string(),
        source_table: source_table.to_string(),
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn resolve_flag_refs(t: &Tables, row: &Value, field: &str) -> Vec<RequirementRef> {
    row.get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|h| {
                    let rn = h.get("RowName")?.as_str()?.to_string();
                    let dtn = h.get("DataTableName").and_then(|v| v.as_str()).unwrap_or("D_CharacterFlags");
                    let table = match dtn {
                        "D_SessionFlags" => &t.session_flags,
                        "D_AccountFlags" => &t.account_flags,
                        _ => &t.character_flags,
                    };
                    let display_name = table
                        .find(&rn)
                        .map(|r| {
                            let desc = text_field(&r, "Description");
                            if desc.is_empty() { rn.clone() } else { desc }
                        })
                        .unwrap_or_else(|| rn.clone());
                    Some(RequirementRef { row: rn, display_name })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn stat_key(raw: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"^\(Value="(.*)"\)$"#).unwrap());
    match re.captures(raw) {
        Some(c) => c[1].to_string(),
        None => raw.to_string(),
    }
}

fn resolve_rewards(row: &Value) -> Vec<RewardInfo> {
    row.get("Rewards")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|entry| {
                    let granted_stats = entry
                        .get("GrantedStats")
                        .and_then(|v| v.as_object())
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (stat_key(k), v.as_f64().unwrap_or(0.0)))
                                .collect()
                        })
                        .unwrap_or_default();
                    let granted_flags = entry
                        .get("GrantedFlags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|h| h.get("RowName").and_then(|v| v.as_str()).map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    RewardInfo { granted_stats, granted_flags }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_tree(t: &Tables, tree_name: &str) -> (String, String, String, Option<String>) {
    let Some(tree) = t.trees.find(tree_name) else {
        return (tree_name.to_string(), String::new(), String::new(), None);
    };
    let tree_display_name = non_empty(text_field(&tree, "DisplayName")).unwrap_or_else(|| tree_name.to_string());
    let tree_icon = opt_str(&tree, "Icon");
    let archetype_row_name = handle_row_name(&tree, "Archetype").unwrap_or_default();
    let archetype_display_name = t
        .archetypes
        .find(&archetype_row_name)
        .map(|a| text_field(&a, "DisplayName"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| archetype_row_name.clone());
    (tree_display_name, archetype_row_name, archetype_display_name, tree_icon)
}

fn classify(archetype: &str, has_display_data: bool) -> Category {
    if !has_display_data {
        return Category::Connector;
    }
    if archetype.starts_with("Blueprint_") {
        Category::Blueprint
    } else if archetype.starts_with("Workshop_") {
        Category::Workshop
    } else if archetype.starts_with("Prospect_") || archetype == "Outpost" || archetype.starts_with("GreatHunt_") {
        Category::Mission
    } else if archetype.starts_with("Creature_") {
        Category::Creature
    } else {
        Category::Talent
    }
}

fn resolve_itemable(t: &Tables, row_name: &str) -> Option<DisplayInfo> {
    let row = t.itemable.find(row_name)?;
    Some(DisplayInfo {
        name: text_field(&row, "DisplayName"),
        description: text_field(&row, "Description"),
        icon: opt_str(&row, "Icon"),
    })
}

fn resolve_workshop_item(t: &Tables, row_name: &str) -> Option<DisplayInfo> {
    let wi = t.workshop_items.find(row_name)?;
    let item_row_name = handle_row_name(&wi, "Item")?;
    let item = t.item_template.find(&item_row_name)?;
    let static_row_name = handle_row_name(&item, "ItemStaticData")?;
    let static_row = t.items_static.find(&static_row_name)?;
    let itemable_row_name = handle_row_name(&static_row, "Itemable")?;
    resolve_itemable(t, &itemable_row_name)
}

fn resolve_outpost(t: &Tables, row_name: &str) -> Option<DisplayInfo> {
    let outpost = t.outposts.find(row_name)?;
    let icon = opt_str(&outpost, "Icon");
    let prospect_row_name = handle_row_name(&outpost, "ProspectListRowHandle");
    let mut info = prospect_row_name
        .and_then(|rn| resolve_prospect(t, &rn))
        .unwrap_or_default();
    if info.icon.is_none() {
        info.icon = icon;
    }
    Some(info)
}

fn resolve_prospect(t: &Tables, row_name: &str) -> Option<DisplayInfo> {
    let row = t.prospect_list.find(row_name)?;
    Some(DisplayInfo {
        name: text_field(&row, "DropName"),
        description: {
            let d = text_field(&row, "Description");
            if d.is_empty() { text_field(&row, "FlavourText") } else { d }
        },
        icon: opt_str(&row, "ProspectImage"),
    })
}

fn resolve_great_hunt(t: &Tables, row_name: &str) -> Option<DisplayInfo> {
    let hunt = t.great_hunts.find(row_name)?;

    // The "Hunt" archetype (e.g. "Quarrite") is shared by every difficulty/
    // stage of the same creature hunt, so using it alone collapses many
    // distinct D_Talents rows onto one identical name. The specific mission
    // instance lives behind "Prospect" instead, so prefer that.
    let archetype_row_name = handle_row_name(&hunt, "Hunt");
    let archetype = archetype_row_name.as_deref().and_then(|rn| t.archetypes.find(rn));
    let creature_name = archetype.as_ref().map(|a| text_field(a, "DisplayName")).unwrap_or_default();
    let archetype_icon = archetype.as_ref().and_then(|a| opt_str(a, "Icon"));

    let prospect_row_name = handle_row_name(&hunt, "Prospect");
    let prospect_info = prospect_row_name.as_deref().and_then(|rn| resolve_prospect(t, rn));

    match prospect_info {
        Some(mut info) => {
            if info.name.is_empty() {
                info.name = creature_name;
            } else if !creature_name.is_empty() {
                info.name = format!("{creature_name}: {}", info.name);
            }
            if info.icon.is_none() {
                info.icon = archetype_icon;
            }
            Some(info)
        }
        None => Some(DisplayInfo {
            name: creature_name,
            description: String::new(),
            icon: archetype_icon,
        }),
    }
}

fn resolve_settlement_building(t: &Tables, row_name: &str) -> Option<DisplayInfo> {
    let row = t.settlement_buildings.find(row_name)?;
    Some(DisplayInfo {
        name: text_field(&row, "BuildingName"),
        description: text_field(&row, "BuildingDescription"),
        icon: None,
    })
}

fn resolve_trees(t: &Tables) -> Vec<ResolvedTree> {
    t.trees
        .rows
        .iter()
        .filter_map(|raw| {
            let name = raw.get("Name")?.as_str()?.to_string();
            let row = t.trees.find(&name)?;
            let display_name = non_empty(text_field(&row, "DisplayName")).unwrap_or_else(|| name.clone());
            let icon = opt_str(&row, "Icon");
            let archetype = handle_row_name(&row, "Archetype").unwrap_or_default();
            Some(ResolvedTree { name, display_name, icon, icon_file: None, archetype })
        })
        .collect()
}

fn resolve_ranks(t: &Tables) -> Vec<ResolvedRank> {
    t.ranks
        .rows
        .iter()
        .filter_map(|raw| {
            let name = raw.get("Name")?.as_str()?.to_string();
            let row = t.ranks.find(&name)?;
            Some(ResolvedRank {
                display_name: non_empty(text_field(&row, "DisplayName")).unwrap_or_else(|| name.clone()),
                investment: i64_field(&row, "Investment"),
                name,
            })
        })
        .collect()
}

fn resolve_character_flags(t: &Tables) -> Vec<ResolvedFlag> {
    resolve_flags(&t.character_flags)
}

fn resolve_account_flags(t: &Tables) -> Vec<ResolvedFlag> {
    resolve_flags(&t.account_flags)
}

fn resolve_flags(table: &DataTable) -> Vec<ResolvedFlag> {
    table
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let name = raw.get("Name")?.as_str()?.to_string();
            let description = raw.get("Description").and_then(|v| v.as_str()).map(nsloctext).unwrap_or_default();
            Some(ResolvedFlag { index, name, description })
        })
        .collect()
}

fn resolve_currencies(t: &Tables) -> Vec<ResolvedCurrency> {
    t.currencies
        .rows
        .iter()
        .filter_map(|raw| {
            let name = raw.get("Name")?.as_str()?.to_string();
            let row = t.currencies.find(&name)?;
            Some(ResolvedCurrency {
                display_name: non_empty(text_field(&row, "DisplayName")).unwrap_or_else(|| name.clone()),
                description: text_field(&row, "Description"),
                icon: opt_str(&row, "Icon"),
                icon_file: None,
                name,
            })
        })
        .collect()
}

/// Extracts and decodes every path in `unique_paths` to a PNG under
/// `out_dir`, returning a map from the raw `/Game/...` icon path to the
/// PNG's filename. Icons that fail to extract/decode are logged and skipped
/// (not a hard error -- the frontend just shows no icon for those).
fn extract_icons<'a>(
    icon_paks: &PakSource,
    unique_paths: impl IntoIterator<Item = &'a str>,
    out_dir: &std::path::Path,
) -> HashMap<String, String> {
    let unique_paths: Vec<&str> = unique_paths.into_iter().collect();
    eprintln!("extracting {} unique icons...", unique_paths.len());
    let mut map = HashMap::new();
    let mut ok = 0;
    let mut failed = 0;
    for game_path in unique_paths {
        match extract_one_icon(icon_paks, game_path, out_dir) {
            Ok(filename) => {
                map.insert(game_path.to_string(), filename);
                ok += 1;
            }
            Err(e) => {
                eprintln!("  icon failed: {game_path}: {e:#}");
                failed += 1;
            }
        }
    }
    eprintln!("icons: {ok} decoded, {failed} failed");
    map
}

fn extract_one_icon(icon_paks: &PakSource, game_path: &str, out_dir: &std::path::Path) -> anyhow::Result<String> {
    let base = game_path_to_asset_base(game_path)
        .ok_or_else(|| anyhow::anyhow!("not a /Game/ path"))?;
    let uasset = icon_paks.read(&format!("{base}.uasset"))?;
    let uexp = icon_paks.read(&format!("{base}.uexp"))?;
    let decoded = texture::decode_texture2d(&uasset, &uexp)?;

    let filename = format!("{}.png", base.replace('/', "_"));
    let out_path = out_dir.join(&filename);

    // The web app renders icons at 44 CSS px; 96 px covers 2x-DPI displays
    // while cutting the shipped asset size to a fraction of the raw
    // 256x256 game textures.
    const MAX_ICON_PX: u32 = 96;
    let img = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba)
        .ok_or_else(|| anyhow::anyhow!("decoded buffer size mismatch"))?;
    let img = if img.width() > MAX_ICON_PX || img.height() > MAX_ICON_PX {
        let scale = MAX_ICON_PX as f64 / img.width().max(img.height()) as f64;
        let w = ((img.width() as f64 * scale).round() as u32).max(1);
        let h = ((img.height() as f64 * scale).round() as u32).max(1);
        image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    img.save(&out_path).map_err(|e| anyhow::anyhow!("PNG encode failed: {e}"))?;
    Ok(filename)
}
