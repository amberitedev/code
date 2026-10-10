//! Optional metadata hashes extend transport integrity without changing upstream file payloads.
use sha2::{Digest, Sha256};

/// Compare live content with a published snapshot when its backend provides a hash.
pub(crate) async fn shared_file_changed(
    path: &std::path::Path,
    expected: Option<&str>,
) -> crate::Result<bool> {
    let Some(expected) = expected else {
        return Ok(false);
    };
    use tokio::io::AsyncReadExt;
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(true);
        }
        Err(error) => return Err(crate::util::io::IOError::from(error).into()),
    };
    let mut digest = Sha256::new();
    // Heap-allocated: an array here would sit inside every caller's future and
    // overflow the async runtime's worker stack in debug builds.
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .await
            .map_err(crate::util::io::IOError::from)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()) != expected)
}

/// Only suppress a recipient update after matching its validated installed path.
pub(crate) async fn installed_shared_file_changed(
    instance_path: &std::path::Path,
    relative_path: Option<&str>,
    expected: Option<&str>,
) -> crate::Result<bool> {
    let (Some(relative_path), Some(expected)) = (relative_path, expected)
    else {
        return Ok(true);
    };
    let Ok(path) = path_util::SafeRelativeUtf8UnixPathBuf::try_from(
        relative_path.to_string(),
    ) else {
        return Ok(true);
    };
    shared_file_changed(&instance_path.join(path.as_str()), Some(expected))
        .await
}

/// Download a shared file. The account session and plain HTTP are used only for
/// the configured sharing service, which serves files to members.
pub(crate) async fn download(url: &str) -> crate::Result<reqwest::Response> {
    let parsed = reqwest::Url::parse(url)?;
    let session = crate::instance::shared_clients_session().filter(|session| {
        reqwest::Url::parse(&session.base_url)
            .is_ok_and(|base| base.origin() == parsed.origin())
    });
    let client = if session.is_some() && parsed.scheme() == "http" {
        &crate::util::fetch::INSECURE_REQWEST_CLIENT
    } else {
        &crate::util::fetch::REQWEST_CLIENT
    };
    let mut request = client.get(url);
    if let Some(token) = session.and_then(|session| session.access_token) {
        request = request.bearer_auth(token);
    }
    Ok(request.send().await.map_err(reqwest::Error::without_url)?)
}

/// Stream a shared file into a temporary file and verify before extracting.
pub(crate) async fn download_to_file(
    url: &str,
    expected: Option<&str>,
) -> crate::Result<tempfile::TempPath> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;
    let response = download(url).await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "Previous config bundle download failed with status {}",
            response.status(),
        ))
        .into());
    }
    let temporary = tempfile::NamedTempFile::new()?.into_temp_path();
    let mut target = tokio::fs::File::create(&temporary)
        .await
        .map_err(crate::util::io::IOError::from)?;
    let mut digest = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(reqwest::Error::without_url)?;
        digest.update(&chunk);
        target
            .write_all(&chunk)
            .await
            .map_err(crate::util::io::IOError::from)?;
    }
    target
        .flush()
        .await
        .map_err(crate::util::io::IOError::from)?;
    if expected.is_some_and(|expected| {
        expected.len() != 64 || format!("{:x}", digest.finalize()) != expected
    }) {
        return Err(crate::ErrorKind::OtherError(
            "Shared file failed its content hash check. Retry the download."
                .to_string(),
        )
        .into());
    }
    Ok(temporary)
}

pub(crate) fn verify_shared_file(
    bytes: &[u8],
    expected: Option<&str>,
) -> crate::Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected.len() != 64
        || format!("{:x}", Sha256::digest(bytes)) != expected
    {
        return Err(crate::ErrorKind::OtherError(
            "Shared file failed its content hash check. Retry the download."
                .to_string(),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn detects_same_name_and_size_replacement_without_changing_legacy_files()
     {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("resource-pack.zip");
        let original = b"original";
        let replacement = b"modified";
        assert_eq!(original.len(), replacement.len());
        let digest = format!("{:x}", Sha256::digest(original));
        tokio::fs::write(&path, original).await.unwrap();
        assert!(!shared_file_changed(&path, Some(&digest)).await.unwrap());
        tokio::fs::write(&path, replacement).await.unwrap();
        assert!(shared_file_changed(&path, Some(&digest)).await.unwrap());
        assert!(!shared_file_changed(&path, None).await.unwrap());
    }

    #[test]
    fn rejects_equal_length_corruption_against_metadata_hash() {
        let digest = format!("{:x}", Sha256::digest(b"original"));
        assert!(verify_shared_file(b"original", Some(&digest)).is_ok());
        assert!(verify_shared_file(b"modified", Some(&digest)).is_err());
        assert!(verify_shared_file(b"original", None).is_ok());
    }

    #[tokio::test]
    async fn recipient_compares_safe_installed_paths_and_keeps_unknown_updates()
    {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        tokio::fs::create_dir_all(instance.join("resourcepacks"))
            .await
            .unwrap();
        let relative = "resourcepacks/pack.zip";
        let path = instance.join(relative);
        let digest = format!("{:x}", Sha256::digest(b"original"));
        tokio::fs::write(&path, b"original").await.unwrap();
        assert!(
            !installed_shared_file_changed(
                &instance,
                Some(relative),
                Some(&digest),
            )
            .await
            .unwrap()
        );
        tokio::fs::write(&path, b"modified").await.unwrap();
        assert!(
            installed_shared_file_changed(
                &instance,
                Some(relative),
                Some(&digest),
            )
            .await
            .unwrap()
        );
        for (relative_path, expected) in [
            (Some(relative), None),
            (None, Some(digest.as_str())),
            (Some("resourcepacks/missing.zip"), Some(digest.as_str())),
            (Some("../pack.zip"), Some(digest.as_str())),
            (Some("C:/pack.zip"), Some(digest.as_str())),
            (Some("resourcepacks\\pack.zip"), Some(digest.as_str())),
        ] {
            assert!(
                installed_shared_file_changed(
                    &instance,
                    relative_path,
                    expected,
                )
                .await
                .unwrap()
            );
        }
    }
}
