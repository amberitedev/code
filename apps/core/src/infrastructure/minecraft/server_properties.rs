use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PropertiesError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid property: {0}")]
    Invalid(String),
}

/// Read `server.properties` from `dir` into a key-value map.
pub async fn read_properties(
    dir: &Path,
) -> Result<HashMap<String, String>, PropertiesError> {
    let path = dir.join("server.properties");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(map)
}

/// Write key-value pairs to `server.properties` in `dir`.
pub async fn write_properties(
    dir: &Path,
    props: &HashMap<String, String>,
) -> Result<(), PropertiesError> {
    validate_properties(props)?;
    let path = dir.join("server.properties");
    let mut content = String::new();
    for (k, v) in props {
        content.push_str(&format!("{k}={v}\n"));
    }
    tokio::fs::write(&path, content).await?;
    Ok(())
}

/// Write the minimal `server.properties` and `eula.txt` for a new instance.
pub async fn write_initial_properties(
    dir: &Path,
    port: u16,
) -> Result<(), PropertiesError> {
    let mut props = HashMap::new();
    props.insert("server-port".to_string(), port.to_string());
    props.insert("online-mode".to_string(), "true".to_string());
    write_properties(dir, &props).await?;
    tokio::fs::write(dir.join("eula.txt"), "eula=true\n").await?;
    Ok(())
}

/// Update specific keys in `server.properties` in-place, preserving comment lines.
/// Returns the list of keys that were updated.
pub async fn patch_properties(
    dir: &Path,
    updates: &HashMap<String, String>,
) -> Result<Vec<String>, PropertiesError> {
    validate_properties(updates)?;
    let path = dir.join("server.properties");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();

    let mut updated_keys: Vec<String> = Vec::new();
    let mut remaining: HashMap<String, String> = updates.clone();
    let mut new_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            new_lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            let key = k.trim().to_string();
            if let Some(new_val) = remaining.remove(&key) {
                new_lines.push(format!("{key}={new_val}"));
                updated_keys.push(key);
                continue;
            }
        }
        new_lines.push(line.to_string());
    }

    // Append keys that weren't found in the file
    for (k, v) in &remaining {
        new_lines.push(format!("{k}={v}"));
        updated_keys.push(k.clone());
    }

    let out = new_lines.join("\n") + "\n";
    tokio::fs::write(&path, out).await?;
    Ok(updated_keys)
}

fn validate_properties(
    props: &HashMap<String, String>,
) -> Result<(), PropertiesError> {
    for (key, value) in props {
        validate_key(key)?;
        validate_value(key, value)?;
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), PropertiesError> {
    if key.is_empty()
        || key.chars().any(|character| {
            !(character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.'))
        })
    {
        return Err(PropertiesError::Invalid(format!("invalid key {key:?}")));
    }
    Ok(())
}

fn validate_value(key: &str, value: &str) -> Result<(), PropertiesError> {
    if value.chars().any(char::is_control) {
        return Err(PropertiesError::Invalid(format!(
            "{key} contains a control character"
        )));
    }
    Ok(())
}
