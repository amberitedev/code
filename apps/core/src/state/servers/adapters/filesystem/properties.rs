//! `server.properties`, read and changed line by line so comments, order, and keys Core does not
//! know survive. `server-port` is Core's: it is set at every start and never changed here.

use std::{collections::BTreeMap, io, path::Path};

use crate::servers::launch::{
    property_key, read_server_file, write_server_file,
};

/// Every key and value in the server folder's `server.properties`. None there is empty.
pub fn read(dir: &Path) -> io::Result<BTreeMap<String, String>> {
    let text = match read_server_file(&dir.join("server.properties")) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    Ok(text
        .lines()
        .filter_map(|line| {
            let key = property_key(line)?;
            let value = line.trim_start()[key.len()..]
                .trim_start()
                .strip_prefix(['=', ':'])
                .unwrap_or_default();
            Some((key.to_owned(), value.trim_start().to_owned()))
        })
        .collect())
}

/// Set (`Some`) or remove (`None`) each key, keeping every other line. `server-port` is skipped.
pub fn patch(
    dir: &Path,
    changes: &BTreeMap<String, Option<String>>,
) -> io::Result<()> {
    let path = dir.join("server.properties");
    let text = match read_server_file(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let changes: BTreeMap<&str, Option<&str>> = changes
        .iter()
        .filter(|(key, _)| key.as_str() != "server-port" && valid_key(key))
        .map(|(key, value)| (key.as_str(), value.as_deref().map(single_line)))
        .collect();
    let mut lines: Vec<String> = Vec::new();
    let mut written = Vec::new();
    for line in text.lines() {
        match property_key(line).and_then(|key| changes.get_key_value(key)) {
            Some((key, Some(value))) => {
                if !written.contains(key) {
                    lines.push(format!("{key}={value}"));
                    written.push(*key);
                }
            }
            Some((_, None)) => {}
            None => lines.push(line.to_owned()),
        }
    }
    for (key, value) in &changes {
        if let Some(value) = value
            && !written.contains(key)
        {
            lines.push(format!("{key}={value}"));
        }
    }
    write_server_file(&path, format!("{}\n", lines.join("\n")).as_bytes())
}

/// A key is one line of letters, digits, dots, dashes, and underscores.
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A value cannot add lines of its own.
fn single_line(value: &str) -> &str {
    value.lines().next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_keeps_other_lines_and_never_the_port() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("server.properties"),
            "#Minecraft server properties\nmotd=Old\nserver-port=25570\ndifficulty=easy\n",
        )
        .unwrap();
        patch(
            dir.path(),
            &BTreeMap::from([
                ("motd".into(), Some("New\nserver-port=1".into())),
                ("difficulty".into(), None),
                ("pvp".into(), Some("false".into())),
                ("server-port".into(), Some("1".into())),
            ]),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("server.properties"))
                .unwrap(),
            "#Minecraft server properties\nmotd=New\nserver-port=25570\npvp=false\n"
        );
        let read = read(dir.path()).unwrap();
        assert_eq!(read.get("motd").map(String::as_str), Some("New"));
        assert_eq!(read.get("server-port").map(String::as_str), Some("25570"));
    }
}
