use std::path::{Path, PathBuf};

use futures::{StreamExt, TryStreamExt};
use sha2::Digest;
use tokio::io::AsyncReadExt;

use super::{CONCURRENCY, PushError, PushFile};
use crate::rules::{SCAN_HEAD_SIZE, is_executable_content};

/// A file as hashed for the push.
pub struct HashedFile {
    pub file_name: String,
    pub file_type: String,
    pub size: u64,
    /// What a member's install compares to skip a file it already has.
    pub sha1: String,
    /// The SHA-256 of the plain bytes.
    pub sha256: [u8; 32],
    pub executable: bool,
    path: PathBuf,
}

impl HashedFile {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(test)]
    pub(super) fn test(file_type: &str, file_name: &str, p: u8) -> Self {
        Self {
            file_name: file_name.into(),
            file_type: file_type.into(),
            size: 1,
            sha1: "sha1".into(),
            sha256: [p; 32],
            executable: false,
            path: PathBuf::new(),
        }
    }
}

pub(super) async fn hash_files(
    files: &[PushFile],
) -> Result<Vec<HashedFile>, PushError> {
    let hashes = files.iter().map(hash_file).collect::<Vec<_>>();
    futures::stream::iter(hashes)
        .buffered(CONCURRENCY)
        .try_collect()
        .await
}

async fn hash_file(file: &PushFile) -> Result<HashedFile, PushError> {
    let mut reader = tokio::fs::File::open(&file.path).await?;
    let mut sha1 = sha1_smol::Sha1::new();
    let mut sha256 = sha2::Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut head = Vec::with_capacity(SCAN_HEAD_SIZE);
    let mut size = 0_u64;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
        let remaining = SCAN_HEAD_SIZE - head.len();
        head.extend_from_slice(&buffer[..read.min(remaining)]);
        size += read as u64;
    }
    Ok(HashedFile {
        file_name: file.file_name.clone(),
        file_type: file.file_type.clone(),
        size,
        sha1: sha1.digest().to_string(),
        sha256: sha256.finalize().into(),
        executable: is_executable_content(&head),
        path: file.path.clone(),
    })
}
