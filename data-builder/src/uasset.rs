//! Minimal parser for the part of Unreal's `.uasset` package header we
//! actually need: the Name Table. Everything else in the header (imports,
//! exports, custom versions, ...) is parsed just enough to skip over it.
//!
//! Verified against Icarus's cooked UE 4.25 packages specifically: they're
//! saved "unversioned" (FileVersionUE/LicenseeUE all zero) with zero custom
//! versions, which fixes the exact set of fields present. This is not a
//! general-purpose uasset reader.

use anyhow::{Context, Result};
use std::io::{Cursor, Read};

pub struct ByteReader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { cursor: Cursor::new(data) }
    }

    pub fn pos(&self) -> u64 {
        self.cursor.position()
    }

    pub fn set_pos(&mut self, pos: u64) {
        self.cursor.set_position(pos);
    }

    pub fn u8(&mut self) -> Result<u8> {
        let mut b = [0u8; 1];
        self.cursor.read_exact(&mut b)?;
        Ok(b[0])
    }

    pub fn i32(&mut self) -> Result<i32> {
        let mut b = [0u8; 4];
        self.cursor.read_exact(&mut b)?;
        Ok(i32::from_le_bytes(b))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(self.i32()? as u32)
    }

    pub fn i64(&mut self) -> Result<i64> {
        let mut b = [0u8; 8];
        self.cursor.read_exact(&mut b)?;
        Ok(i64::from_le_bytes(b))
    }

    pub fn bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let mut b = vec![0u8; n];
        self.cursor.read_exact(&mut b)?;
        Ok(b)
    }

    pub fn bool32(&mut self) -> Result<bool> {
        Ok(self.i32()? != 0)
    }

    /// Reads a length-prefixed `FString`: a positive length means ASCII/UTF-8
    /// (including the trailing NUL), negative means UTF-16 (also
    /// NUL-terminated).
    pub fn fstring(&mut self) -> Result<String> {
        let len = self.i32()?;
        if len == 0 {
            return Ok(String::new());
        }
        if len > 0 {
            let b = self.bytes(len as usize)?;
            Ok(String::from_utf8_lossy(&b[..b.len().saturating_sub(1)]).to_string())
        } else {
            let n = (-len) as usize;
            let b = self.bytes(n * 2)?;
            let u16s: Vec<u16> = b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            Ok(String::from_utf16_lossy(&u16s[..u16s.len().saturating_sub(1)]))
        }
    }
}

/// Parses just the Name Table out of a `.uasset` file's bytes.
pub fn read_name_table(uasset_bytes: &[u8]) -> Result<Vec<String>> {
    let mut r = ByteReader::new(uasset_bytes);

    let tag = r.u32().context("tag")?;
    const PACKAGE_FILE_TAG: u32 = 0x9E2A83C1;
    anyhow::ensure!(tag == PACKAGE_FILE_TAG, "not a uasset file (bad magic {tag:#x})");

    let legacy_version = r.i32().context("legacyFileVersion")?;
    if legacy_version != -4 {
        r.i32().context("ue3 version")?; // FileVersionUE3, unused
    }
    r.i32().context("ue4 version")?; // FileVersionUE4, unused (Icarus packages are unversioned)
    if legacy_version <= -8 {
        r.i32().context("ue5 version")?;
    }
    r.i32().context("licensee version")?;

    // CustomVersionContainer: Icarus's cooked packages always carry zero
    // custom versions, serialized as just a count.
    let cv_count = r.i32().context("custom version count")?;
    for _ in 0..cv_count.max(0) {
        r.bytes(16)?; // Guid
        r.i32()?; // Version
    }

    r.i32().context("total header size")?;
    r.fstring().context("package name")?;
    r.u32().context("package flags")?;

    let name_count = r.i32().context("name count")?;
    let name_offset = r.i32().context("name offset")?;

    r.set_pos(name_offset as u64);
    let mut names = Vec::with_capacity(name_count.max(0) as usize);
    for i in 0..name_count {
        let s = r.fstring().with_context(|| format!("name[{i}]"))?;
        r.bytes(4)?; // two u16 hashes (non-case-preserving, case-preserving)
        names.push(s);
    }
    Ok(names)
}
