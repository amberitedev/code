//! Reading a `.mrpack`'s index out of the archive.

use std::io::{Read, Seek};

use zip::ZipArchive;

use crate::content::model::mrpack::Index;

/// The index is small; a pack claiming a larger one is refused.
const MAX_INDEX: u64 = 32 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("this is not a Modrinth pack: {0}")]
    NotAPack(String),
}

/// Read and check a pack's `modrinth.index.json`.
pub fn read_index<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Index, IndexError> {
    let not_a_pack = |reason: &str| IndexError::NotAPack(reason.to_owned());
    let mut text = String::new();
    archive
        .by_name("modrinth.index.json")
        .map_err(|_| not_a_pack("it has no modrinth.index.json"))?
        .take(MAX_INDEX)
        .read_to_string(&mut text)
        .map_err(|_| not_a_pack("its index is unreadable"))?;
    let index: Index = serde_json::from_str(&text)
        .map_err(|error| IndexError::NotAPack(error.to_string()))?;
    if index.format_version != Index::FORMAT_VERSION
        || index.game != Index::GAME
    {
        return Err(not_a_pack("it is not a Minecraft pack of format 1"));
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn pack(index: &str) -> ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("modrinth.index.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(index.as_bytes()).unwrap();
        ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    #[test]
    fn only_minecraft_packs_of_format_1_are_read() {
        let index = read_index(&mut pack(
            r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Pack","dependencies":{"minecraft":"1.21.1"}}"#,
        ))
        .unwrap();
        assert_eq!(index.name, "Pack");
        assert!(
            read_index(&mut pack(
                r#"{"formatVersion":2,"game":"minecraft","versionId":"1","name":"Pack"}"#
            ))
            .is_err()
        );
        assert!(read_index(&mut pack("{")).is_err());
    }
}
