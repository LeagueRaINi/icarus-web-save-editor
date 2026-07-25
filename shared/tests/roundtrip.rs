//! Round-trip verification against real save files. Point the env vars at
//! copies of your own saves (they are not committed to the repo):
//!
//!   ICARUS_CHARACTERS=path\to\Characters.json
//!   ICARUS_PROFILE=path\to\Profile.json
//!
//! Each test is skipped when the file isn't present, so `cargo test` stays
//! green on machines without the game installed.

use shared::save::{
    parse_characters_file, parse_profile_file, serialize_characters_file, serialize_profile_file,
};

fn read_or_skip(env: &str, test: &str) -> Option<String> {
    let path = std::env::var(env).ok()?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("skipping {test}: cannot read {path}: {e}");
            None
        }
    }
}

/// Points at the first byte where two strings differ, with context, so a
/// formatting regression is immediately visible.
fn assert_identical(ours: &str, theirs: &str, what: &str) {
    if ours == theirs {
        return;
    }
    let pos = ours
        .bytes()
        .zip(theirs.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| ours.len().min(theirs.len()));
    let ctx = |s: &str| {
        let start = pos.saturating_sub(60);
        let end = (pos + 60).min(s.len());
        s.get(start..end).unwrap_or("<non-utf8 boundary>").to_string()
    };
    panic!(
        "{what}: output differs from the game's own bytes at offset {pos}\n\
         ours:   ...{:?}...\n\
         theirs: ...{:?}...\n\
         (lengths: ours {} vs theirs {})",
        ctx(ours),
        ctx(theirs),
        ours.len(),
        theirs.len()
    );
}

#[test]
fn characters_roundtrip_real_save() {
    let Some(raw) = read_or_skip("ICARUS_CHARACTERS", "characters_roundtrip_real_save") else {
        return;
    };
    let characters = parse_characters_file(&raw).expect("real Characters.json should parse");
    assert!(!characters.is_empty());
    let out = serialize_characters_file(&characters).expect("serialize");

    // Semantic equality: nothing lost, renamed, or re-typed.
    let a: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let b: serde_json::Value = serde_json::from_str(&out).unwrap();
    let inner = |v: &serde_json::Value| -> Vec<serde_json::Value> {
        v["Characters.json"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| serde_json::from_str(s.as_str().unwrap()).unwrap())
            .collect()
    };
    assert_eq!(inner(&a), inner(&b), "character contents must survive the round-trip");

    // Byte equality: same tabs, CRLF, field order as the game itself.
    assert_identical(&out, &raw, "Characters.json");
}

#[test]
fn profile_roundtrip_real_save() {
    let Some(raw) = read_or_skip("ICARUS_PROFILE", "profile_roundtrip_real_save") else {
        return;
    };
    let profile = parse_profile_file(&raw).expect("real Profile.json should parse");
    let out = serialize_profile_file(&profile).expect("serialize");

    let a: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let b: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(a, b, "profile contents must survive the round-trip");

    assert_identical(&out, &raw, "Profile.json");
}
