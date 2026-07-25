use anyhow::{Context, Result};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// Icarus's exported datatable JSON escapes apostrophes as `\'`, which is not
/// a valid JSON escape sequence and makes strict parsers (including
/// serde_json) fail. A naive `"\\'".replace()` is unsafe: it also mangles
/// *legitimately* escaped backslashes that happen to precede an apostrophe
/// (e.g. `\\'` = escaped-backslash + plain-apostrophe, valid JSON on its
/// own), turning them into another invalid `\'`. So this walks the string
/// respecting escape sequences: a backslash only ever "consumes" the next
/// character if that pair forms a recognized (or Unreal's bogus `\'`)
/// escape, which keeps already-escaped backslashes intact.
fn sanitize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some(n @ ('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't')) => {
                out.push('\\');
                out.push(n);
                chars.next();
            }
            Some('u') => {
                out.push('\\');
                out.push('u');
                chars.next();
                for _ in 0..4 {
                    if let Some(h) = chars.next() {
                        out.push(h);
                    }
                }
            }
            Some('\'') => {
                // Unreal's bogus escape: drop the backslash, keep the apostrophe.
                out.push('\'');
                chars.next();
            }
            Some(other) => {
                // Any other invalid escape: drop the backslash defensively.
                out.push(other);
                chars.next();
            }
            None => out.push('\\'),
        }
    }
    out
}

pub struct DataTable {
    pub defaults: Value,
    pub rows: Vec<Value>,
}

impl DataTable {
    /// Loads a `D_*.json` datatable straight out of the game's Data pak
    /// (see `pak_source::PakSource`) -- `entry_path` is pak-internal, e.g.
    /// `"Talents/D_Talents.json"`.
    pub fn load(source: &crate::pak_source::PakSource, entry_path: &str) -> Result<Self> {
        let raw = source
            .read_to_string(entry_path)
            .with_context(|| format!("reading {entry_path}"))?;
        let cleaned = sanitize(&raw);
        let value: Value =
            serde_json::from_str(&cleaned).with_context(|| format!("parsing {entry_path}"))?;
        let defaults = value.get("Defaults").cloned().unwrap_or(Value::Null);
        let rows = value
            .get("Rows")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(Self { defaults, rows })
    }

    /// Look up a row by its `Name` field and merge it over `Defaults`,
    /// so callers can read a field without checking both the row and
    /// the table's defaults every time.
    pub fn find(&self, name: &str) -> Option<Value> {
        let row = self.rows.iter().find(|r| r.get("Name").and_then(|n| n.as_str()) == Some(name))?;
        Some(merge_over_defaults(&self.defaults, row))
    }
}

fn merge_over_defaults(defaults: &Value, row: &Value) -> Value {
    match (defaults, row) {
        (Value::Object(def_map), Value::Object(row_map)) => {
            let mut merged = def_map.clone();
            for (k, v) in row_map {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        _ => row.clone(),
    }
}

/// Extracts the literal display text out of an Unreal `NSLOCTEXT("Namespace",
/// "Key", "The Actual Text")` or `INVTEXT("The Actual Text")` call. Falls
/// back to the input unchanged if neither matches (e.g. already-plain
/// strings, or "None"/empty). Also strips a leading "[DNT]" marker ("do not
/// translate" -- an internal authoring note, not part of the display text)
/// that shows up on a few INVTEXT entries.
pub fn nsloctext(raw: &str) -> String {
    static NSLOCTEXT_RE: OnceLock<Regex> = OnceLock::new();
    static INVTEXT_RE: OnceLock<Regex> = OnceLock::new();
    let nsloctext_re = NSLOCTEXT_RE.get_or_init(|| {
        Regex::new(r#"(?s)^NSLOCTEXT\(\s*"[^"]*"\s*,\s*"[^"]*"\s*,\s*"(.*)"\s*\)\s*$"#).unwrap()
    });
    let invtext_re = INVTEXT_RE.get_or_init(|| Regex::new(r#"(?s)^INVTEXT\(\s*"(.*)"\s*\)\s*$"#).unwrap());

    let trimmed = raw.trim();
    let extracted = if let Some(caps) = nsloctext_re.captures(trimmed) {
        caps[1].to_string()
    } else if let Some(caps) = invtext_re.captures(trimmed) {
        caps[1].to_string()
    } else {
        return raw.to_string();
    };
    let extracted = unescape_text_literal(&extracted);
    extracted.strip_prefix("[DNT] ").map(str::to_string).unwrap_or(extracted)
}

/// The text body inside an NSLOCTEXT/INVTEXT call is a C-style escaped
/// string literal: quotes and apostrophes arrive as `\"` / `\'`. Resolve
/// those so display text doesn't show stray backslashes.
fn unescape_text_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Convenience: read a string field off a merged row, running it through
/// `nsloctext`. Returns "" if missing/not a string.
pub fn text_field(row: &Value, field: &str) -> String {
    row.get(field)
        .and_then(|v| v.as_str())
        .map(nsloctext)
        .unwrap_or_default()
}

pub fn opt_str(row: &Value, field: &str) -> Option<String> {
    row.get(field)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "None")
        .map(normalize_icon)
}

/// Most icon fields are plain content paths (`/Game/...`), but a few (like
/// `D_ProspectList.ProspectImage`) store a full Unreal object reference
/// (`Texture2D'/Game/...'`). Strip that wrapper so every icon string the
/// frontend sees has the same plain-path shape.
fn normalize_icon(raw: &str) -> String {
    if let Some(quote_pos) = raw.find('\'') {
        if raw.ends_with('\'') && quote_pos > 0 {
            return raw[quote_pos + 1..raw.len() - 1].to_string();
        }
    }
    raw.to_string()
}

/// Reads a `{ "RowName": "...", "DataTableName": "..." }` handle. Returns
/// None if RowName is missing/"None".
pub fn handle_row_name(row: &Value, field: &str) -> Option<String> {
    row.get(field)
        .and_then(|h| h.get("RowName"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "None")
        .map(|s| s.to_string())
}

pub fn i64_field(row: &Value, field: &str) -> i64 {
    row.get(field).and_then(|v| v.as_i64()).unwrap_or(0)
}

pub fn bool_field(row: &Value, field: &str) -> bool {
    row.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
}
