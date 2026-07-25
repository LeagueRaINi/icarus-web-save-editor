//! Serialization matching the exact on-disk format the game itself writes,
//! so an edited file diffs cleanly against the original (verified
//! byte-identical against real saves in `tests/roundtrip.rs`):
//!
//! - inner escaped character strings: tab indent, LF newlines, all arrays
//!   multi-line (the game's inner serializer);
//! - outer files (Characters.json wrapper, Profile.json): tab indent, CRLF
//!   newlines, and numeric arrays inlined as `[ 1, 2, 3 ]` (the game's
//!   outer serializer).

use serde::Serialize;
use serde_json::Value;

/// Tab-indented, LF newlines -- the format Icarus uses inside the escaped
/// strings of Characters.json.
pub fn to_string_tabbed<T: Serialize>(value: &T) -> Result<String, String> {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Tab-indented, CRLF, numeric arrays inline -- the outer save-file format.
pub fn to_string_tabbed_crlf<T: Serialize>(value: &T) -> Result<String, String> {
    let v = serde_json::to_value(value).map_err(|e| e.to_string())?;
    let mut out = String::new();
    write_value(&mut out, &v, 0);
    Ok(out)
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push('\t');
    }
}

fn write_value(out: &mut String, v: &Value, depth: usize) {
    match v {
        Value::Array(a) if a.is_empty() => out.push_str("[]"),
        // The game inlines arrays of plain numbers: `[ 86, 5, 26 ]`.
        Value::Array(a) if a.iter().all(Value::is_number) => {
            out.push_str("[ ");
            for (i, e) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&e.to_string());
            }
            out.push_str(" ]");
        }
        Value::Array(a) => {
            out.push_str("[\r\n");
            for (i, e) in a.iter().enumerate() {
                indent(out, depth + 1);
                write_value(out, e, depth + 1);
                if i + 1 < a.len() {
                    out.push(',');
                }
                out.push_str("\r\n");
            }
            indent(out, depth);
            out.push(']');
        }
        Value::Object(m) if m.is_empty() => out.push_str("{}"),
        Value::Object(m) => {
            out.push_str("{\r\n");
            for (i, (k, e)) in m.iter().enumerate() {
                indent(out, depth + 1);
                out.push_str(&Value::String(k.clone()).to_string());
                out.push_str(": ");
                write_value(out, e, depth + 1);
                if i + 1 < m.len() {
                    out.push(',');
                }
                out.push_str("\r\n");
            }
            indent(out, depth);
            out.push('}');
        }
        // Strings get serde_json's escaping (same as the game's), numbers /
        // bools / null print canonically.
        other => out.push_str(&other.to_string()),
    }
}
