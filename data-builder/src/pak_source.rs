use anyhow::{Context, Result};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Read-only access across one or more `.pak` files.
///
/// Every pak's index is merged into one entry -> pak lookup at open time, so
/// a read goes straight to the pak that actually contains the entry rather
/// than probing each one in turn. Which pak holds a given asset is decided by
/// the pak indexes themselves, not by filename or ordering conventions.
///
/// Backed by `repak`, which (unlike the generic community UnrealPak.exe
/// build) has its own Oodle decompression support built in, so it can read
/// Icarus's content paks directly with no external tool.
pub struct PakSource {
    paks: Vec<(repak::PakReader, RefCell<BufReader<File>>)>,
    /// Content-relative path -> the pak holding it and the mount-relative
    /// entry path `repak` wants back when reading it.
    entries: HashMap<String, (usize, String)>,
}

impl PakSource {
    pub fn open(pak_paths: &[impl AsRef<Path>]) -> Result<Self> {
        let mut paks = Vec::new();
        let mut entries: HashMap<String, (usize, String)> = HashMap::new();
        let mut collisions: Vec<String> = Vec::new();

        for path in pak_paths {
            let path = path.as_ref();
            let mut reader = BufReader::new(
                File::open(path).with_context(|| format!("opening {}", path.display()))?,
            );
            let pak = repak::PakBuilder::new()
                .reader(&mut reader)
                .with_context(|| format!("reading pak index of {}", path.display()))?;

            let mount_point = pak.mount_point().to_string();
            for entry in pak.files() {
                match entries.entry(content_relative(&mount_point, &entry)) {
                    Entry::Occupied(slot) => collisions.push(slot.key().clone()),
                    Entry::Vacant(slot) => {
                        slot.insert((paks.len(), entry));
                    }
                }
            }

            paks.push((pak, RefCell::new(reader)));
        }

        // Should be empty: distinct assets get distinct content-relative
        // paths. If a game update ever does ship the same one twice, say so
        // rather than silently reading whichever came first.
        if !collisions.is_empty() {
            eprintln!(
                "warning: {} entries resolve to a path another pak already owns, e.g. {:?}",
                collisions.len(),
                &collisions[..collisions.len().min(3)],
            );
        }
        Ok(Self { paks, entries })
    }

    /// Reads one entry's raw (decompressed) bytes. `path` is relative to
    /// `Icarus/Content`, e.g. "Talents/D_Talents.json" or
    /// "Assets/2DArt/UI/Items/Item_Icons/Tools/ITEM_Stone_Axe.uexp".
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let (index, entry) = self
            .entries
            .get(path)
            .with_context(|| format!("entry not found in any opened pak: {path}"))?;
        let (pak, reader) = &self.paks[*index];
        pak.get(entry, &mut *reader.borrow_mut())
            .with_context(|| format!("reading {path}"))
    }

    pub fn read_to_string(&self, path: &str) -> Result<String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).with_context(|| format!("{path} is not valid UTF-8"))
    }

}

/// Rebases one pak entry onto `Icarus/Content`, the namespace the rest of this
/// tool works in (and what a `/Game/...` reference maps onto).
///
/// Each pak declares a mount point that its index entries are relative to, and
/// those mount points differ: `Icarus/Content/` for s19/s20, but also
/// `Icarus/Content/ASS/`, `Icarus/` and `Icarus/Content/Maps/Terrain_016_OLY/`.
/// Keying on the raw entry path would therefore both miss assets (an
/// `ASS/`-mounted pak stores `DEP/T_Foo.uasset`, which no `ASS/DEP/T_Foo.uasset`
/// lookup can match) and conflate unrelated ones (Olympus and Styx each store
/// `HeightMap/HeightMap_x7_y4.uexp`).
///
/// `data.pak` is an outlier: its mount point is a leftover absolute path from
/// the build machine, so there is nothing to rebase onto and its already
/// Content-style entries are used as they are.
fn content_relative(mount_point: &str, entry: &str) -> String {
    const CONTENT: &str = "Icarus/Content/";
    let separator = if mount_point.ends_with('/') { "" } else { "/" };
    let full = format!("{mount_point}{separator}{entry}");
    match full.find(CONTENT) {
        Some(at) => full[at + CONTENT.len()..].to_string(),
        None => entry.to_string(),
    }
}

/// Converts a `/Game/...` content reference (as seen in DisplayName/Icon
/// fields throughout the D_*.json tables) into the Content-relative path of
/// its `.uasset`/`.uexp` pair, e.g.
/// `/Game/Assets/2DArt/UI/Icons/Icon_Foo.Icon_Foo` ->
/// `Assets/2DArt/UI/Icons/Icon_Foo`.
pub fn game_path_to_asset_base(game_path: &str) -> Option<String> {
    let path = game_path.strip_prefix("/Game/")?;
    let dot = path.rfind('.')?;
    Some(path[..dot].to_string())
}

#[cfg(test)]
mod tests {
    use super::content_relative;

    #[test]
    fn rebases_onto_content() {
        // Mount points as they actually appear in Icarus's paks.
        assert_eq!(
            content_relative("../../../Icarus/Content/", "ASS/VFX/NS_Foo.uasset"),
            "ASS/VFX/NS_Foo.uasset"
        );
        assert_eq!(
            content_relative("../../../Icarus/Content/ASS/", "DEP/T_Crate.uasset"),
            "ASS/DEP/T_Crate.uasset"
        );
        assert_eq!(
            content_relative("../../../Icarus/", "Content/TRD/Anim.uasset"),
            "TRD/Anim.uasset"
        );
    }

    #[test]
    fn keeps_same_named_terrain_apart() {
        let olympus = content_relative(
            "../../../Icarus/Content/Maps/Terrain_016_OLY/",
            "HeightMap/HeightMap_x7_y4.uexp",
        );
        let styx = content_relative(
            "../../../Icarus/Content/Maps/Terrain_017_STYX/",
            "HeightMap/HeightMap_x7_y4.uexp",
        );
        assert_eq!(olympus, "Maps/Terrain_016_OLY/HeightMap/HeightMap_x7_y4.uexp");
        assert_ne!(olympus, styx);
    }

    #[test]
    fn falls_back_when_mount_is_not_under_content() {
        // data.pak's mount point is a build-machine temp directory.
        assert_eq!(
            content_relative("C:/BA/work/92bbbfa44df12262/Temp/Data/", "Talents/D_Talents.json"),
            "Talents/D_Talents.json"
        );
    }
}
