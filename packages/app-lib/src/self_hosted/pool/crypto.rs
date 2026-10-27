//! The pool's file encryption, exactly as `docs/internals/pool.md` (Protocol) writes it down. The
//! backend and the store check the same values, so nothing here may change on its own.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::store::StoreError;

/// Plain bytes per chunk.
pub const CHUNK: usize = 1024 * 1024;
const TAG: usize = 16;
/// Object bytes per chunk, for every chunk but the last.
const OBJECT_CHUNK: usize = CHUNK + TAG;

/// The key of a content file, derived from its plain hash P so the same jar is the same object
/// whoever shares it.
pub fn content_key(p: &[u8; 32]) -> [u8; 32] {
    let mut key = [0; 32];
    hkdf::Hkdf::<Sha256>::new(Some(&[]), p)
        .expand(b"amberite-shared-file-v1", &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 length");
    key
}

/// Content files get a derived key; every other file type gets a random one.
pub fn is_content_type(file_type: &str) -> bool {
    matches!(file_type, "mod" | "resourcepack" | "shader" | "datapack")
}

/// The encrypted size of a plain file of `size` bytes.
pub fn object_size(size: u64) -> u64 {
    size + TAG as u64 * size.div_ceil(CHUNK as u64).max(1)
}

/// R: the hash of a version's distinct objects, sorted, as raw bytes.
pub fn root(objects: impl IntoIterator<Item = [u8; 32]>) -> [u8; 32] {
    let objects = objects
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let mut hasher = Sha256::new();
    for object in objects {
        hasher.update(object);
    }
    hasher.finalize().into()
}

fn nonce(chunk: u64) -> XNonce {
    let mut nonce = XNonce::default();
    nonce[..8].copy_from_slice(&chunk.to_be_bytes());
    nonce
}

/// Reads until `size` bytes are in or the reader ends.
async fn read_up_to(
    reader: &mut (impl AsyncRead + Unpin),
    size: usize,
) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(size);
    (&mut *reader)
        .take(size as u64)
        .read_to_end(&mut buffer)
        .await?;
    Ok(buffer)
}

/// Encrypts a plain file into its object, returning C and the object's size. Deterministic for a
/// given key, so a retried upload can encrypt again.
pub async fn encrypt(
    key: &[u8; 32],
    mut reader: impl AsyncRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
) -> std::io::Result<([u8; 32], u64)> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut hasher = Sha256::new();
    let mut size = 0;
    let mut chunk = 0;
    let mut current = read_up_to(&mut reader, CHUNK).await?;
    loop {
        // A full chunk is the last only when nothing follows it.
        let next = if current.len() == CHUNK {
            read_up_to(&mut reader, CHUNK).await?
        } else {
            Vec::new()
        };
        let last = next.is_empty();
        let object = cipher
            .encrypt(
                &nonce(chunk),
                Payload {
                    msg: &current,
                    aad: &[u8::from(last)],
                },
            )
            .map_err(|_| std::io::Error::other("encryption failed"))?;
        hasher.update(&object);
        writer.write_all(&object).await?;
        size += object.len() as u64;
        if last {
            writer.flush().await?;
            return Ok((hasher.finalize().into(), size));
        }
        chunk += 1;
        current = next;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PoolFetchError {
    /// The bytes are not the object that was asked for: the Core's fault.
    #[error("the Core sent bytes that are not the shared file")]
    WrongObject,
    /// The object decrypts to a different file than the manifest names: the manifest's fault.
    #[error("the shared file does not match its manifest")]
    WrongContent,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Http(#[from] StoreError),
}

/// Decrypts and verifies one object. Keeps its place between [`Decryptor::run`] calls, so a
/// broken download continues from [`Decryptor::offset`] with the same checks.
pub struct Decryptor {
    cipher: XChaCha20Poly1305,
    chunk: u64,
    object: Sha256,
    plain: Sha256,
    expected_object: [u8; 32],
    expected_plain: [u8; 32],
}

impl Decryptor {
    pub fn new(
        key: &[u8; 32],
        expected_object: [u8; 32],
        expected_plain: [u8; 32],
    ) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(key.into()),
            chunk: 0,
            object: Sha256::new(),
            plain: Sha256::new(),
            expected_object,
            expected_plain,
        }
    }

    /// The object byte to fetch from next.
    pub fn offset(&self) -> u64 {
        self.chunk * OBJECT_CHUNK as u64
    }

    /// Streams the object from [`Self::offset`] on. Every chunk is checked before its plain
    /// bytes are written, and the last chunk is written only after C and P both match, so a
    /// writer that requires the full size never sees a wrong file complete.
    pub async fn run(
        &mut self,
        mut reader: impl AsyncRead + Unpin,
        mut writer: impl AsyncWrite + Unpin,
    ) -> Result<(), PoolFetchError> {
        let mut current = read_up_to(&mut reader, OBJECT_CHUNK).await?;
        loop {
            let next = if current.len() == OBJECT_CHUNK {
                read_up_to(&mut reader, OBJECT_CHUNK).await?
            } else {
                Vec::new()
            };
            let last = next.is_empty();
            let plain = self
                .cipher
                .decrypt(
                    &nonce(self.chunk),
                    Payload {
                        msg: &current,
                        aad: &[u8::from(last)],
                    },
                )
                .map_err(|_| PoolFetchError::WrongObject)?;
            self.object.update(&current);
            self.plain.update(&plain);
            self.chunk += 1;
            if last {
                if self.object.clone().finalize()[..] != self.expected_object {
                    return Err(PoolFetchError::WrongObject);
                }
                if self.plain.clone().finalize()[..] != self.expected_plain {
                    return Err(PoolFetchError::WrongContent);
                }
                writer.write_all(&plain).await?;
                writer.flush().await?;
                return Ok(());
            }
            writer.write_all(&plain).await?;
            current = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];

    fn plain(size: usize) -> Vec<u8> {
        (0..size).map(|index| (index % 251) as u8).collect()
    }

    async fn encrypted(plain: &[u8]) -> ([u8; 32], Vec<u8>) {
        let mut object = Vec::new();
        let (c, size) = encrypt(&KEY, plain, &mut object).await.unwrap();
        assert_eq!(size, object.len() as u64);
        (c, object)
    }

    async fn decrypt(
        object: &[u8],
        c: [u8; 32],
        p: [u8; 32],
    ) -> Result<Vec<u8>, PoolFetchError> {
        let mut out = Vec::new();
        Decryptor::new(&KEY, c, p).run(object, &mut out).await?;
        Ok(out)
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    #[tokio::test]
    async fn round_trips_at_every_chunk_edge() {
        for size in [0, 1, CHUNK, CHUNK + 1, 3 * CHUNK] {
            let plain = plain(size);
            let (c, object) = encrypted(&plain).await;
            assert_eq!(object.len() as u64, object_size(size as u64));
            assert_eq!(c, sha256(&object));
            assert_eq!(
                decrypt(&object, c, sha256(&plain)).await.unwrap(),
                plain
            );
        }
    }

    #[tokio::test]
    async fn same_key_and_file_give_the_same_object() {
        let plain = plain(CHUNK + 5);
        assert_eq!(encrypted(&plain).await, encrypted(&plain).await);
    }

    #[tokio::test]
    async fn resumes_at_a_chunk() {
        let plain = plain(2 * CHUNK + 3);
        let (c, object) = encrypted(&plain).await;
        let mut decryptor = Decryptor::new(&KEY, c, sha256(&plain));
        let mut out = Vec::new();
        // The connection drops partway into the second chunk: that chunk reads as a short
        // last chunk and fails its tag, leaving the place at the chunk's start.
        let cut = &object[..OBJECT_CHUNK + 100];
        assert!(decryptor.run(cut, &mut out).await.is_err());
        assert_eq!(decryptor.offset(), OBJECT_CHUNK as u64);
        let rest = &object[decryptor.offset() as usize..];
        decryptor.run(rest, &mut out).await.unwrap();
        assert_eq!(out, plain);
    }

    #[tokio::test]
    async fn any_changed_byte_is_a_wrong_object() {
        let plain = plain(2 * CHUNK + 10);
        let (c, object) = encrypted(&plain).await;
        for index in [0, CHUNK, OBJECT_CHUNK + 3, object.len() - 1] {
            let mut tampered = object.clone();
            tampered[index] ^= 1;
            assert!(matches!(
                decrypt(&tampered, c, sha256(&plain)).await,
                Err(PoolFetchError::WrongObject)
            ));
        }
    }

    #[tokio::test]
    async fn truncation_and_reordering_fail() {
        let plain = plain(3 * CHUNK);
        let (c, object) = encrypted(&plain).await;
        let p = sha256(&plain);
        for cut in [0, 10, OBJECT_CHUNK, 2 * OBJECT_CHUNK, object.len() - 1] {
            assert!(decrypt(&object[..cut], c, p).await.is_err());
        }
        let mut reordered = object[OBJECT_CHUNK..2 * OBJECT_CHUNK].to_vec();
        reordered.extend_from_slice(&object[..OBJECT_CHUNK]);
        reordered.extend_from_slice(&object[2 * OBJECT_CHUNK..]);
        assert!(matches!(
            decrypt(&reordered, c, p).await,
            Err(PoolFetchError::WrongObject)
        ));
    }

    #[tokio::test]
    async fn a_wrong_plain_hash_is_wrong_content() {
        let plain = plain(10);
        let (c, object) = encrypted(&plain).await;
        let mut out = Vec::new();
        let result = Decryptor::new(&KEY, c, [0; 32])
            .run(&object[..], &mut out)
            .await;
        assert!(matches!(result, Err(PoolFetchError::WrongContent)));
        // The held-back last chunk never reaches the writer.
        assert!(out.is_empty());
    }

    #[test]
    fn object_sizes() {
        let chunk = CHUNK as u64;
        assert_eq!(object_size(0), 16);
        assert_eq!(object_size(1), 17);
        assert_eq!(object_size(chunk), chunk + 16);
        assert_eq!(object_size(chunk + 1), chunk + 1 + 32);
        assert_eq!(object_size(3 * chunk), 3 * chunk + 48);
    }

    #[test]
    fn root_hashes_distinct_sorted_objects() {
        let (a, b) = ([1; 32], [2; 32]);
        let mut expected = a.to_vec();
        expected.extend_from_slice(&b);
        assert_eq!(root([b, a, b]), sha256(&expected));
        assert_eq!(
            hex::encode(root([])),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn content_keys_follow_the_protocol() {
        assert!(is_content_type("mod") && is_content_type("datapack"));
        assert!(!is_content_type("config") && !is_content_type("configs"));
        // An empty HKDF salt is the same as 32 zero bytes (RFC 5869), which is what a
        // WebCrypto or Node backend computing the same key will use.
        let p = [9; 32];
        let mut expected = [0; 32];
        hkdf::Hkdf::<Sha256>::new(Some(&[0; 32]), &p)
            .expand(b"amberite-shared-file-v1", &mut expected)
            .unwrap();
        assert_eq!(content_key(&p), expected);
    }
}
