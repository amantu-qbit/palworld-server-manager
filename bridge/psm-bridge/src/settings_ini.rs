//! Read and edit `PalWorldSettings.ini`.
//!
//! The whole server config lives in one line inside
//! `[/Script/Pal.PalGameWorldSettings]`:
//!
//! ```text
//! OptionSettings=(Difficulty=None,DayTimeSpeedRate=1.000000,ServerName="My, Server",bIsPvP=False,...)
//! ```
//!
//! We locate that parenthesised tuple (quote- and paren-aware, so a
//! `ServerName`/`ServerDescription` containing commas or parens is respected),
//! rewrite only the keys the caller changed, and splice the result back — every
//! other byte of the file (comments, other sections, unchanged keys, their exact
//! number formatting) is preserved verbatim. No INI crate; the format is stable.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A single OptionSettings entry, as surfaced to the app.
#[derive(Debug, Clone, PartialEq)]
pub struct IniSetting {
    pub key: String,
    /// Logical value — surrounding quotes stripped for string entries.
    pub value: String,
    /// True when the on-disk token was a double-quoted string.
    pub quoted: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum IniError {
    #[error("read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("PalWorldSettings.ini has no OptionSettings=(…) line")]
    NoOptionSettings,
}

/// Byte range of the tuple *inside* the `OptionSettings=(...)` parentheses.
fn inner_span(raw: &str) -> Option<(usize, usize)> {
    const MARKER: &str = "OptionSettings=(";
    let start = raw.find(MARKER)? + MARKER.len();
    let bytes = raw.as_bytes();
    let mut depth = 1i32;
    let mut in_quote = false;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] as char {
            '"' => in_quote = !in_quote,
            '(' if !in_quote => depth += 1,
            ')' if !in_quote => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, i));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split the tuple body into `(key, raw_token)` pairs on top-level commas,
/// keeping each value's exact on-disk token (quotes and all).
fn split_pairs(inner: &str) -> Vec<(String, String)> {
    let bytes = inner.as_bytes();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut in_quote = false;
    let mut depth = 0i32;
    let mut seg_start = 0usize;

    let push = |out: &mut Vec<(String, String)>, seg: &str| {
        let seg = seg.trim();
        if seg.is_empty() {
            return;
        }
        if let Some(eq) = seg.find('=') {
            out.push((seg[..eq].trim().to_string(), seg[eq + 1..].trim().to_string()));
        }
    };

    for i in 0..bytes.len() {
        match bytes[i] as char {
            '"' => in_quote = !in_quote,
            '(' if !in_quote => depth += 1,
            ')' if !in_quote => depth -= 1,
            ',' if !in_quote && depth == 0 => {
                push(&mut out, &inner[seg_start..i]);
                seg_start = i + 1;
            }
            _ => {}
        }
    }
    push(&mut out, &inner[seg_start..]);
    out
}

fn unquote(token: &str) -> (String, bool) {
    if token.len() >= 2 && token.starts_with('"') && token.ends_with('"') {
        (token[1..token.len() - 1].to_string(), true)
    } else {
        (token.to_string(), false)
    }
}

/// Format a caller's logical value back into an on-disk token. `was_quoted`
/// carries the original entry's shape; a brand-new key is quoted unless it looks
/// like a number or bool. Internal quotes are stripped (Palworld tokens don't
/// escape them) so the tuple can't be broken.
fn to_token(value: &str, was_quoted: Option<bool>) -> String {
    let quoted = match was_quoted {
        Some(q) => q,
        None => !(value == "True"
            || value == "False"
            || value.parse::<f64>().is_ok()),
    };
    if quoted {
        format!("\"{}\"", value.replace('"', ""))
    } else {
        value.to_string()
    }
}

/// Parse the OptionSettings entries from raw file text.
pub fn parse(raw: &str) -> Result<Vec<IniSetting>, IniError> {
    let (s, e) = inner_span(raw).ok_or(IniError::NoOptionSettings)?;
    Ok(split_pairs(&raw[s..e])
        .into_iter()
        .map(|(key, token)| {
            let (value, quoted) = unquote(&token);
            IniSetting { key, value, quoted }
        })
        .collect())
}

/// Rewrite the OptionSettings tuple with `changes` (logical values) applied.
/// Unchanged entries keep their exact original token; keys not present are
/// appended. Everything outside the tuple is preserved byte-for-byte.
pub fn apply(raw: &str, changes: &BTreeMap<String, String>) -> Result<String, IniError> {
    let (s, e) = inner_span(raw).ok_or(IniError::NoOptionSettings)?;
    let mut pairs = split_pairs(&raw[s..e]);
    let mut applied: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for (key, token) in pairs.iter_mut() {
        if let Some(new_val) = changes.get(key) {
            let (_, was_quoted) = unquote(token);
            *token = to_token(new_val, Some(was_quoted));
            applied.insert(key.clone());
        }
    }
    for (key, val) in changes {
        if !applied.contains(key) {
            pairs.push((key.clone(), to_token(val, None)));
        }
    }

    let body = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{}{}{}", &raw[..s], body, &raw[e..]))
}

/// Read + parse a settings file at `path`.
pub fn read(path: &Path) -> Result<(String, Vec<IniSetting>), IniError> {
    let raw = std::fs::read_to_string(path).map_err(|source| IniError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let settings = parse(&raw)?;
    Ok((raw, settings))
}

/// Copy `path` into `<parent>/psm-backups/<name>.<UTC>.<ext>` before overwrite,
/// preserving the original extension (unlike the `.sav`-specific save backup),
/// keeping the newest 20. Returns the backup path.
pub fn backup(path: &Path) -> Result<PathBuf, IniError> {
    let io = |source: std::io::Error| IniError::Io {
        path: path.to_path_buf(),
        source,
    };
    let parent = path.parent().ok_or_else(|| IniError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other("no parent dir"),
    })?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("settings");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("ini");

    let dir = parent.join("psm-backups");
    std::fs::create_dir_all(&dir).map_err(io)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| IniError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::other("clock error"),
        })?;
    let backup = dir.join(format!("{stem}.{}.{:03}.{ext}", now.as_secs(), now.subsec_millis()));
    std::fs::copy(path, &backup).map_err(io)?;

    prune(&dir, stem, ext);
    Ok(backup)
}

fn prune(dir: &Path, stem: &str, ext: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let prefix = format!("{stem}.");
    let suffix = format!(".{ext}");
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(&prefix) && n.ends_with(&suffix))
        .collect();
    names.sort();
    if names.len() > 20 {
        for n in &names[..names.len() - 20] {
            let _ = std::fs::remove_file(dir.join(n));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(Difficulty=None,DayTimeSpeedRate=1.000000,bIsPvP=False,ServerName=\"My, (cool) Server\",ServerPlayerMaxNum=32,AdminPassword=\"secret\")\n[OtherSection]\nFoo=Bar\n";

    #[test]
    fn parses_entries_including_quoted_commas_and_parens() {
        let s = parse(SAMPLE).unwrap();
        let get = |k: &str| s.iter().find(|e| e.key == k).unwrap();
        assert_eq!(get("Difficulty").value, "None");
        assert!(!get("Difficulty").quoted);
        assert_eq!(get("DayTimeSpeedRate").value, "1.000000");
        assert_eq!(get("bIsPvP").value, "False");
        // Comma + parens inside the quoted string must not split the entry.
        assert_eq!(get("ServerName").value, "My, (cool) Server");
        assert!(get("ServerName").quoted);
        assert_eq!(get("AdminPassword").value, "secret");
    }

    #[test]
    fn apply_changes_only_named_keys_and_preserves_the_rest() {
        let mut ch = BTreeMap::new();
        ch.insert("Difficulty".to_string(), "Hard".to_string());
        ch.insert("bIsPvP".to_string(), "True".to_string());
        ch.insert("ServerName".to_string(), "New Name".to_string());
        let out = apply(SAMPLE, &ch).unwrap();

        assert!(out.contains("Difficulty=Hard"));
        assert!(out.contains("bIsPvP=True"));
        // String stays quoted; unchanged number keeps its exact formatting.
        assert!(out.contains("ServerName=\"New Name\""));
        assert!(out.contains("DayTimeSpeedRate=1.000000"));
        // Everything outside the tuple is untouched.
        assert!(out.contains("[OtherSection]\nFoo=Bar"));
        // Re-parsing the result round-trips.
        let re = parse(&out).unwrap();
        assert_eq!(re.iter().find(|e| e.key == "Difficulty").unwrap().value, "Hard");
    }

    #[test]
    fn apply_appends_a_missing_key_with_inferred_quoting() {
        let mut ch = BTreeMap::new();
        ch.insert("bIsUseBackupSaveData".to_string(), "True".to_string());
        ch.insert("Region".to_string(), "eu".to_string());
        let out = apply(SAMPLE, &ch).unwrap();
        assert!(out.contains("bIsUseBackupSaveData=True")); // bool → bare
        assert!(out.contains("Region=\"eu\"")); // non-numeric/bool → quoted
    }

    #[test]
    fn missing_option_settings_errors() {
        assert!(matches!(parse("[X]\nFoo=Bar\n"), Err(IniError::NoOptionSettings)));
    }
}
