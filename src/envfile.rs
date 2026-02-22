use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

static KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("valid regex"));

pub fn load(path: &Path) -> Result<HashMap<String, String>, std::io::Error> {
    let text = fs::read_to_string(path)?;
    parse(&text).map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

pub fn parse(text: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();

    for (index, raw) in text.lines().enumerate() {
        let mut line = raw.trim_end_matches('\r').trim().to_string();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(stripped) = line.strip_prefix("export ") {
            line = stripped.trim().to_string();
        }

        let Some(cut) = line.find('=') else {
            return Err(format!("line {}: expected KEY=VALUE", index + 1));
        };

        if cut == 0 {
            return Err(format!("line {}: expected KEY=VALUE", index + 1));
        }

        let key = line[..cut].trim();
        if !KEY_RE.is_match(key) {
            return Err(format!("line {}: invalid key {key:?}", index + 1));
        }

        let value = parse_value(line[cut + 1..].trim())
            .map_err(|err| format!("line {}: {err}", index + 1))?;

        out.insert(key.to_string(), value);
    }

    Ok(out)
}

fn parse_value(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }

    if value.starts_with('"') {
        if !value.ends_with('"') || value.len() == 1 {
            return Err("unterminated double-quoted value".to_string());
        }

        let quoted = serde_json::from_str::<String>(value)
            .map_err(|_| "invalid double-quoted value".to_string())?;
        return Ok(quoted);
    }

    if value.starts_with('\'') {
        if !value.ends_with('\'') || value.len() == 1 {
            return Err("unterminated single-quoted value".to_string());
        }
        return Ok(value[1..value.len() - 1].to_string());
    }

    let trimmed = if let Some(idx) = value.find(" #") {
        value[..idx].trim().to_string()
    } else {
        value.to_string()
    };

    Ok(trimmed)
}
