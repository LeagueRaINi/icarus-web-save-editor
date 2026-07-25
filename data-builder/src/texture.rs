//! Decodes an Unreal `Texture2D` asset (`.uasset` + `.uexp`, no `.ubulk` --
//! every icon this game ships is small enough to be fully inline) into a PNG.
//!
//! There is no crate that does this for us: `unreal_asset` parses generic
//! tagged properties but has no notion of `Texture2D`'s custom binary tail,
//! and `texture2ddecoder` only turns already-extracted BCn/DXT blocks into
//! pixels. So this hand-rolls just enough of Unreal's serialization format,
//! ported from Unreal Engine's own (MIT-licensed, reverse-engineered)
//! CUE4Parse reference implementation and verified byte-for-byte against
//! real extracted Icarus icons (see data-builder development notes) -- most
//! of the mip-chain/bulk-data plumbing below only exists because there was
//! no shortcut around reimplementing it.
//!
//! One part is empirical rather than derived: exactly 12 bytes of
//! strip-data-flags + `bCooked` precede the pixel format name, confirmed
//! identical across every icon sampled. Full semantic derivation of those
//! bytes (matching them to UTexture/UTexture2D's serialize order) didn't
//! line up cleanly with the reference source for this cook, but the gap
//! being a fixed constant is all that matters for correctness here.

use crate::uasset::ByteReader;
use anyhow::{bail, ensure, Context, Result};

struct TexReader<'a> {
    r: ByteReader<'a>,
    names: &'a [String],
}

impl<'a> TexReader<'a> {
    fn fname(&mut self) -> Result<String> {
        let idx = self.r.i32()?;
        let number = self.r.i32()?;
        let base = self
            .names
            .get(idx as usize)
            .cloned()
            .with_context(|| format!("FName index {idx} out of range ({} names)", self.names.len()))?;
        Ok(if number == 0 { base } else { format!("{base}_{}", number - 1) })
    }
}

/// Skips one `FPropertyTag` (legacy/UE4 tagged-property format). Returns
/// `false` once the "None" terminator is hit. We only need the tag's fixed
/// header fields (to know how many extra bytes an entry like StructProperty
/// or ByteProperty needs) -- the tag's own `Size` field lets us skip the
/// actual value blob blindly without interpreting it.
fn skip_property(t: &mut TexReader) -> Result<bool> {
    let name = t.fname()?;
    if name == "None" {
        return Ok(false);
    }
    let prop_type = t.fname()?;
    let size = t.r.i32()?;
    let _array_index = t.r.i32()?;

    match prop_type.as_str() {
        "StructProperty" => {
            t.fname()?; // StructType
            t.r.bytes(16)?; // StructGuid
        }
        "BoolProperty" => {
            t.r.u8()?; // inline bool value
        }
        "ByteProperty" | "EnumProperty" => {
            t.fname()?; // EnumName
        }
        "ArrayProperty" => {
            t.fname()?; // InnerType
        }
        "SetProperty" => {
            t.fname()?; // InnerType
        }
        "MapProperty" => {
            t.fname()?; // key type
            t.fname()?; // value type
        }
        _ => {}
    }

    let has_guid = t.r.u8()? != 0;
    if has_guid {
        t.r.bytes(16)?;
    }

    let value_start = t.r.pos();
    t.r.set_pos(value_start + size as u64);
    Ok(true)
}

pub struct DecodedTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Parses a `Texture2D`'s `.uexp` tail and decodes its largest (first) mip
/// to RGBA8. `uasset_bytes` is only needed for its Name Table.
pub fn decode_texture2d(uasset_bytes: &[u8], uexp_bytes: &[u8]) -> Result<DecodedTexture> {
    let names = crate::uasset::read_name_table(uasset_bytes)?;
    let mut t = TexReader { r: ByteReader::new(uexp_bytes), names: &names };

    while skip_property(&mut t)? {}

    // Empirically-constant gap (see module docs): UTexture's + UTexture2D's
    // FStripDataFlags (2 bytes each) and bCooked (4 bytes).
    t.r.bytes(12)?;

    let pixel_format_name = t.fname()?;
    ensure!(pixel_format_name != "None", "texture has no cooked platform data");
    let _skip_offset = t.r.i64()?;

    let _size_x = t.r.i32()?; // FTexturePlatformData's own SizeX/SizeY -- redundant with mip[0]'s
    let _size_y = t.r.i32()?;
    let packed_data = t.r.u32()?;
    let pixel_format = t.r.fstring()?;

    let has_opt_data = (packed_data & (1 << 30)) != 0;
    if has_opt_data {
        t.r.bytes(8)?; // FOptTexturePlatformData { ExtData, NumMipsInTail }
    }

    let _first_mip_to_serialize = t.r.i32()?;
    let mip_count = t.r.i32()?;
    ensure!(mip_count > 0, "texture has no mips");

    // We only need the largest (first) mip for an icon; later/smaller mips
    // are still walked (to make sure our byte-accounting stays correct) but
    // their payloads are skipped rather than decoded.
    let mut first_mip: Option<(i32, i32, Vec<u8>)> = None;
    for m in 0..mip_count {
        let _mip_cooked = t.r.bool32()?;

        let flags = t.r.u32()?;
        const BULKDATA_SIZE_64BIT: u32 = 1 << 13;
        let size64 = (flags & BULKDATA_SIZE_64BIT) != 0;
        let _element_count = if size64 { t.r.i64()? } else { t.r.i32()? as i64 };
        let size_on_disk = if size64 { t.r.i64()? } else { t.r.i32()? as i64 };
        let _offset_in_file = t.r.i64()?;

        let payload = if size_on_disk > 0 { t.r.bytes(size_on_disk as usize)? } else { Vec::new() };

        let mip_size_x = t.r.i32()?;
        let mip_size_y = t.r.i32()?;
        let _mip_size_z = t.r.i32()?;

        if m == 0 {
            first_mip = Some((mip_size_x, mip_size_y, payload));
        }
    }

    let (width, height, payload) = first_mip.context("no mips read")?;
    let rgba = decode_block_compressed(&pixel_format, width as usize, height as usize, &payload)?;

    Ok(DecodedTexture { width: width as u32, height: height as u32, rgba })
}

fn decode_block_compressed(pixel_format: &str, width: usize, height: usize, data: &[u8]) -> Result<Vec<u8>> {
    // A few small UI icons aren't block-compressed at all (below whatever
    // size threshold the cook pipeline applies compression at).
    if pixel_format == "PF_B8G8R8A8" {
        ensure!(data.len() >= width * height * 4, "PF_B8G8R8A8 payload too short");
        let mut rgba = vec![0u8; width * height * 4];
        for i in 0..width * height {
            rgba[i * 4] = data[i * 4 + 2]; // R
            rgba[i * 4 + 1] = data[i * 4 + 1]; // G
            rgba[i * 4 + 2] = data[i * 4]; // B
            rgba[i * 4 + 3] = data[i * 4 + 3]; // A
        }
        return Ok(rgba);
    }

    let mut argb = vec![0u32; width * height];
    match pixel_format {
        "PF_DXT1" => texture2ddecoder::decode_bc1(data, width, height, &mut argb),
        "PF_DXT5" => texture2ddecoder::decode_bc3(data, width, height, &mut argb),
        "PF_BC7" => texture2ddecoder::decode_bc7(data, width, height, &mut argb),
        "PF_BC5" => texture2ddecoder::decode_bc5(data, width, height, &mut argb),
        "PF_BC4" => texture2ddecoder::decode_bc4(data, width, height, &mut argb),
        other => bail!("unsupported pixel format {other}"),
    }
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    // texture2ddecoder packs each pixel as 0xAARRGGBB in host-native u32;
    // as little-endian bytes that's [B, G, R, A].
    let mut rgba = vec![0u8; argb.len() * 4];
    for (i, px) in argb.iter().enumerate() {
        let b = px.to_le_bytes();
        rgba[i * 4] = b[2];
        rgba[i * 4 + 1] = b[1];
        rgba[i * 4 + 2] = b[0];
        rgba[i * 4 + 3] = b[3];
    }
    Ok(rgba)
}
