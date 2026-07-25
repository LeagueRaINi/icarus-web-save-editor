use anyhow::{bail, Context, Result};
use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Read-only access across one or more `.pak` files, trying each in turn.
/// Backed by `repak`, which (unlike the generic community UnrealPak.exe
/// build) has its own Oodle decompression support built in, so it can read
/// Icarus's content paks directly with no external tool.
pub struct PakSource {
    paks: Vec<(repak::PakReader, RefCell<BufReader<File>>)>,
}

impl PakSource {
    pub fn open(pak_paths: &[impl AsRef<Path>]) -> Result<Self> {
        let mut paks = Vec::new();
        for path in pak_paths {
            let path = path.as_ref();
            let mut reader = BufReader::new(
                File::open(path).with_context(|| format!("opening {}", path.display()))?,
            );
            let pak = repak::PakBuilder::new()
                .reader(&mut reader)
                .with_context(|| format!("reading pak index of {}", path.display()))?;
            paks.push((pak, RefCell::new(reader)));
        }
        Ok(Self { paks })
    }

    /// Reads one entry's raw (decompressed) bytes, trying each opened pak in
    /// order. `path` is pak-internal, e.g. "Talents/D_Talents.json" or
    /// "Assets/2DArt/UI/Items/Item_Icons/Tools/ITEM_Stone_Axe.uexp".
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        for (pak, reader) in &self.paks {
            if let Ok(data) = pak.get(path, &mut *reader.borrow_mut()) {
                return Ok(data);
            }
        }
        bail!("entry not found in any opened pak: {path}")
    }

    pub fn read_to_string(&self, path: &str) -> Result<String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).with_context(|| format!("{path} is not valid UTF-8"))
    }

}

/// Converts a `/Game/...` content reference (as seen in DisplayName/Icon
/// fields throughout the D_*.json tables) into the pak-internal relative
/// path of its `.uasset`/`.uexp` pair, e.g.
/// `/Game/Assets/2DArt/UI/Icons/Icon_Foo.Icon_Foo` ->
/// `Assets/2DArt/UI/Icons/Icon_Foo`.
pub fn game_path_to_asset_base(game_path: &str) -> Option<String> {
    let path = game_path.strip_prefix("/Game/")?;
    let dot = path.rfind('.')?;
    Some(path[..dot].to_string())
}
