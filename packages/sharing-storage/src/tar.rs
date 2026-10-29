//! A strict reader for the uncompressed tar an upload streams (see `routes::upload`): regular
//! files only, ustar or GNU headers, names of at most 100 bytes, and the two empty blocks that
//! end it. Hand-written because the store needs one async pass over a request body and nothing
//! else the format allows.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, Take};

const BLOCK: u64 = 512;

/// One file in the tar.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    /// Without a leading `./`.
    pub name: String,
    pub size: u64,
}

pub struct Reader<R> {
    /// Limited to what the current read may take: a header, or the current entry's body.
    inner: Take<R>,
    /// Zeros after the current entry's body, up to the next block.
    padding: u64,
    finished: bool,
}

fn invalid(description: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, description.to_owned())
}

impl<R: AsyncRead + Unpin> Reader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner: inner.take(0),
            padding: 0,
            finished: false,
        }
    }

    /// The next entry, skipping whatever is left of the one before. `None` once the tar ends.
    pub async fn next(&mut self) -> io::Result<Option<Entry>> {
        if self.finished {
            return Ok(None);
        }
        let rest = self.inner.limit() + self.padding;
        self.inner.set_limit(rest);
        let skipped =
            tokio::io::copy(&mut self.inner, &mut tokio::io::sink()).await?;
        if skipped != rest {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.padding = 0;
        let mut header = [0u8; BLOCK as usize];
        self.inner.set_limit(BLOCK);
        self.inner.read_exact(&mut header).await?;
        if header.iter().all(|&byte| byte == 0) {
            self.inner.set_limit(BLOCK);
            self.inner.read_exact(&mut header).await?;
            if header.iter().any(|&byte| byte != 0) {
                return Err(invalid(
                    "The tar ends with one empty block, not two.",
                ));
            }
            self.finished = true;
            self.inner.set_limit(0);
            return Ok(None);
        }
        let entry = parse_header(&header)?;
        self.inner.set_limit(entry.size);
        self.padding = (BLOCK - entry.size % BLOCK) % BLOCK;
        Ok(Some(entry))
    }

    /// The current entry's body. It ends after the entry's size, or earlier when the stream
    /// does, so the caller checks how much it got.
    pub fn body(&mut self) -> &mut Take<R> {
        &mut self.inner
    }
}

fn parse_header(header: &[u8; BLOCK as usize]) -> io::Result<Entry> {
    let stored = octal(&header[148..156])?;
    let sum: u64 = header
        .iter()
        .enumerate()
        .map(|(index, &byte)| {
            // The checksum field counts as spaces.
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(byte)
            }
        })
        .sum();
    if stored != sum {
        return Err(invalid("A tar header's checksum is wrong."));
    }
    let ustar = match &header[257..265] {
        b"ustar\x0000" => true,
        b"ustar  \x00" => false,
        _ => return Err(invalid("The tar is not in ustar or GNU format.")),
    };
    if !matches!(header[156], b'0' | 0) {
        return Err(invalid("The tar may hold only regular files."));
    }
    // GNU keeps other fields where ustar keeps the name's prefix.
    if ustar && header[345] != 0 {
        return Err(invalid("A name in the tar is longer than 100 bytes."));
    }
    let name = &header[..100];
    let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(100)];
    let name = std::str::from_utf8(name)
        .map_err(|_| invalid("A name in the tar is not UTF-8."))?;
    Ok(Entry {
        name: name.strip_prefix("./").unwrap_or(name).to_owned(),
        size: octal(&header[124..136])?,
    })
}

/// An octal number field, padded with spaces or NULs. GNU's base-256 sizes, which only files
/// over 8 GiB need, are refused.
fn octal(field: &[u8]) -> io::Result<u64> {
    let text = std::str::from_utf8(field)
        .map_err(|_| invalid("A tar header has an unreadable number."))?
        .trim_matches(|c| c == ' ' || c == '\0');
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text, 8)
        .map_err(|_| invalid("A tar header has an unreadable number."))
}

/// Tars as an upload sends them, for tests.
#[cfg(test)]
pub mod test_support {
    /// A tar of `entries` as the `tar` crate writes it, with ustar headers (`gnu` false) or GNU
    /// ones.
    pub fn tar(entries: &[(&str, &[u8])], gnu: bool) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, data) in entries {
            let mut header = if gnu {
                tar::Header::new_gnu()
            } else {
                tar::Header::new_ustar()
            };
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            builder.append_data(&mut header, name, *data).unwrap();
        }
        builder.into_inner().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::tar, *};

    async fn read_all(bytes: &[u8]) -> io::Result<Vec<(Entry, Vec<u8>)>> {
        let mut reader = Reader::new(bytes);
        let mut entries = Vec::new();
        while let Some(entry) = reader.next().await? {
            let mut body = Vec::new();
            reader.body().read_to_end(&mut body).await?;
            entries.push((entry, body));
        }
        Ok(entries)
    }

    #[tokio::test]
    async fn the_tar_crate_s_output_reads_back() {
        for gnu in [false, true] {
            let bytes = tar(
                &[
                    ("version.json", b"{}"),
                    ("./abc", &[7u8; 1000]),
                    ("empty", b""),
                ],
                gnu,
            );
            let entries = read_all(&bytes).await.unwrap();
            let names: Vec<_> = entries
                .iter()
                .map(|(entry, _)| entry.name.as_str())
                .collect();
            assert_eq!(names, ["version.json", "abc", "empty"]);
            assert_eq!(entries[1].1, vec![7u8; 1000]);
            assert_eq!(entries[1].0.size, 1000);
            assert!(entries[2].1.is_empty());
        }
    }

    #[tokio::test]
    async fn an_unread_body_is_skipped() {
        let bytes = tar(&[("a", &[1u8; 700]), ("b", b"two")], false);
        let mut reader = Reader::new(bytes.as_slice());
        assert_eq!(reader.next().await.unwrap().unwrap().name, "a");
        let mut part = [0u8; 10];
        reader.body().read_exact(&mut part).await.unwrap();
        assert_eq!(reader.next().await.unwrap().unwrap().name, "b");
        let mut body = String::new();
        reader.body().read_to_string(&mut body).await.unwrap();
        assert_eq!(body, "two");
        assert!(reader.next().await.unwrap().is_none());
        assert!(reader.next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn broken_tars_are_refused() {
        let good = tar(&[("a", b"hello")], false);
        // Cut inside the body, and before the end marker.
        assert!(read_all(&good[..515]).await.is_err());
        assert!(read_all(&good[..1024]).await.is_err());
        // One empty block and then something else.
        let mut one_block = good[..1024].to_vec();
        one_block.extend([0u8; 512]);
        one_block.extend([1u8; 512]);
        assert!(read_all(&one_block).await.is_err());
        // A changed byte breaks the checksum.
        let mut tampered = good.clone();
        tampered[0] = b'b';
        assert!(read_all(&tampered).await.is_err());
        // Not a tar at all.
        assert!(read_all(&[b'x'; 1024]).await.is_err());
    }

    #[tokio::test]
    async fn only_short_named_regular_files_are_accepted() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        builder
            .append_data(&mut header, "folder/", std::io::empty())
            .unwrap();
        let directory = builder.into_inner().unwrap();
        assert!(read_all(&directory).await.is_err());

        // ustar keeps a long name's start in the prefix field.
        let long = format!("{}/{}", "d".repeat(80), "f".repeat(80));
        let bytes = tar(&[(long.as_str(), b"x")], false);
        assert!(read_all(&bytes).await.is_err());
    }
}
