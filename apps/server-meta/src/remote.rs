use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_RANGE, RANGE};
use serde::de::DeserializeOwned;

/// Where the libraries a server needs are published, most preferred first: Modrinth's mirror
/// and Mojang are where an install already downloads from.
const LIBRARY_HOSTS: &[&str] = &[
    "https://launcher-meta.modrinth.com/maven/",
    "https://libraries.minecraft.net/",
    "https://maven.minecraftforge.net/",
    "https://maven.neoforged.net/releases/",
    "https://repo1.maven.org/maven2/",
];

/// HTTP for the generator: retried requests, and a cache of where each library is hosted.
pub struct Remote {
    client: Client,
    hosts: Mutex<HashMap<String, Option<&'static str>>>,
}

impl Remote {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent("amberite-server-meta")
                .timeout(Duration::from_secs(60))
                .build()?,
            hosts: Mutex::default(),
        })
    }

    /// `None` when `url` does not exist.
    fn send(
        &self,
        url: &str,
        request: impl Fn(&Client) -> reqwest::blocking::RequestBuilder,
    ) -> Result<Option<Response>> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match request(&self.client).send() {
                // Some mirrors answer a missing file with 403 or 405 rather than 404.
                Ok(response)
                    if response.status().is_client_error()
                        && response.status()
                            != StatusCode::TOO_MANY_REQUESTS =>
                {
                    return Ok(None);
                }
                Ok(response) if response.status().is_success() => {
                    return Ok(Some(response));
                }
                Ok(response) if attempt >= 4 => {
                    bail!("{url} answered {}", response.status())
                }
                Err(error) if attempt >= 4 => {
                    return Err(error).context(format!("fetching {url}"));
                }
                _ => std::thread::sleep(Duration::from_secs(attempt)),
            }
        }
    }

    pub fn json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.send(url, |client| client.get(url))?
            .with_context(|| format!("{url} does not exist"))?
            .json()
            .with_context(|| format!("reading {url}"))
    }

    /// The archive at `url`, read in ranges so only the entries asked for are downloaded.
    pub fn archive(
        &self,
        url: &str,
    ) -> Result<Option<zip::ZipArchive<RemoteFile<'_>>>> {
        let Some(response) = self
            .send(url, |client| client.get(url).header(RANGE, "bytes=0-0"))?
        else {
            return Ok(None);
        };
        let len = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|range| range.to_str().ok())
            .and_then(|range| range.rsplit_once('/'))
            .and_then(|(_, len)| len.parse().ok())
            .with_context(|| format!("{url} cannot be read in ranges"))?;
        let file = RemoteFile {
            remote: self,
            url: url.to_owned(),
            len,
            pos: 0,
            chunks: HashMap::new(),
        };
        Ok(Some(
            zip::ZipArchive::new(file)
                .with_context(|| format!("reading {url}"))?,
        ))
    }

    /// The repository that hosts the maven `path`, as a base URL a library entry takes.
    pub fn library_host(&self, path: &str) -> Result<Option<&'static str>> {
        if let Some(host) = self.hosts.lock().unwrap().get(path) {
            return Ok(*host);
        }
        let mut found = None;
        for host in LIBRARY_HOSTS {
            let url = format!("{host}{path}");
            if self.send(&url, |client| client.head(&url))?.is_some() {
                found = Some(*host);
                break;
            }
        }
        self.hosts.lock().unwrap().insert(path.to_owned(), found);
        Ok(found)
    }
}

const CHUNK: u64 = 64 * 1024;

/// A remote file read through HTTP ranges, a chunk at a time.
pub struct RemoteFile<'a> {
    remote: &'a Remote,
    url: String,
    len: u64,
    pos: u64,
    chunks: HashMap<u64, Vec<u8>>,
}

impl RemoteFile<'_> {
    fn chunk(&mut self, index: u64) -> io::Result<&[u8]> {
        if !self.chunks.contains_key(&index) {
            let start = index * CHUNK;
            let end = (start + CHUNK).min(self.len) - 1;
            let bytes = self
                .remote
                .send(&self.url, |client| {
                    client
                        .get(&self.url)
                        .header(RANGE, format!("bytes={start}-{end}"))
                })
                .and_then(|response| {
                    let response = response.context("the file disappeared")?;
                    if response.status() != StatusCode::PARTIAL_CONTENT {
                        bail!("the host ignored the range");
                    }
                    let bytes = response.bytes()?.to_vec();
                    if bytes.len() as u64 != end - start + 1 {
                        bail!("the host sent a short range");
                    }
                    Ok(bytes)
                })
                .map_err(io::Error::other)?;
            self.chunks.insert(index, bytes);
        }
        Ok(&self.chunks[&index])
    }
}

impl Read for RemoteFile<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let pos = self.pos;
        let chunk = self.chunk(pos / CHUNK)?;
        let offset = (pos % CHUNK) as usize;
        let n = buf.len().min(chunk.len() - offset);
        buf[..n].copy_from_slice(&chunk[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for RemoteFile<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let pos = match from {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::End(delta) => self.len.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
        };
        self.pos = pos.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "seek before the start")
        })?;
        Ok(self.pos)
    }
}
