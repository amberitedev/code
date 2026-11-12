use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

static THUMBNAIL_WORKER: Semaphore = Semaphore::const_new(1);
static GENERATED_THUMBNAILS: AtomicUsize = AtomicUsize::new(0);
const CACHE_LIMIT: u64 = 128 * 1024 * 1024;

/// The thumbnail of the image at `path` in `cache`, made once per version of the image.
pub async fn thumbnail(
    path: PathBuf,
    cache: PathBuf,
    size: u32,
) -> std::io::Result<PathBuf> {
    let size = size.clamp(32, 512);
    let metadata = tokio::fs::metadata(&path).await?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hash = Sha256::new();
    hash.update(path.as_os_str().as_encoded_bytes());
    hash.update(metadata.len().to_le_bytes());
    hash.update(modified.as_nanos().to_le_bytes());
    hash.update(size.to_le_bytes());
    let hash: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let target = cache.join(format!("{hash}.png"));
    // A cache hit skips the worker queue: every queued request holds one of the page's six
    // connections to the local server.
    if tokio::fs::metadata(&target)
        .await
        .is_ok_and(|target| target.is_file())
    {
        return Ok(target);
    }
    let permit = THUMBNAIL_WORKER
        .acquire()
        .await
        .map_err(std::io::Error::other)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        create_thumbnail(&path, &cache, &target, size)
    })
    .await
    .map_err(std::io::Error::other)?
}

fn create_thumbnail(
    path: &Path,
    cache: &Path,
    target: &Path,
    size: u32,
) -> std::io::Result<PathBuf> {
    // Another request may have made it while this one waited for the worker.
    if target.is_file() {
        return Ok(target.to_owned());
    }
    std::fs::create_dir_all(cache)?;
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let image = reader.decode().map_err(std::io::Error::other)?;
    let thumbnail = image.thumbnail(size, size);
    drop(image);
    let temporary = target.with_extension("tmp");
    thumbnail
        .save_with_format(&temporary, image::ImageFormat::Png)
        .map_err(std::io::Error::other)?;
    std::fs::rename(&temporary, target)?;
    if GENERATED_THUMBNAILS
        .fetch_add(1, Ordering::Relaxed)
        .is_multiple_of(32)
        && let Err(error) = prune_cache(cache, target)
    {
        tracing::warn!(%error, "Could not prune thumbnail cache");
    }
    Ok(target.to_owned())
}

fn prune_cache(cache: &Path, current: &Path) -> std::io::Result<()> {
    let mut entries = Vec::new();
    let mut total = 0;
    for entry in std::fs::read_dir(cache)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        total += metadata.len();
        entries.push((
            metadata.modified().unwrap_or(UNIX_EPOCH),
            metadata.len(),
            entry.path(),
        ));
    }
    entries.sort_unstable_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in entries {
        if total <= CACHE_LIMIT {
            break;
        }
        if path != current && std::fs::remove_file(path).is_ok() {
            total -= size;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{THUMBNAIL_WORKER, thumbnail};

    #[tokio::test]
    async fn a_cached_thumbnail_does_not_wait_for_the_worker() {
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("screenshot.png");
        image::RgbImage::new(64, 64).save(&source).unwrap();
        let cache = folder.path().join("thumbnails");
        let made = thumbnail(source.clone(), cache.clone(), 32).await.unwrap();

        let _busy = THUMBNAIL_WORKER.acquire().await.unwrap();
        let cached = tokio::time::timeout(
            Duration::from_secs(5),
            thumbnail(source, cache, 32),
        )
        .await
        .expect("a cache hit waited for the worker")
        .unwrap();
        assert_eq!(cached, made);
    }
}
