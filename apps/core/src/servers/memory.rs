//! A server's memory and JVM flags. Java starts with a small heap, may grow it to the server's
//! memory, and with the tuned flags G1 hands unused memory back, so an idle server shrinks. The
//! tuned flags are the default; a server can have its own instead (`jvm_flags`), and resetting
//! goes back to the tuned ones.

use sqlx::SqlitePool;

/// The heap a server starts with. It grows to the server's memory as needed.
const START_HEAP_MB: i64 = 512;

/// The default flags. `IgnoreUnrecognizedVMOptions` first, so an older Java that lacks one of
/// them still starts. `G1PeriodicGCInterval` collects an idle server once a minute, and the free
/// ratios make G1 return what it does not need.
pub const TUNED_FLAGS: [&str; 8] = [
    "-XX:+IgnoreUnrecognizedVMOptions",
    "-XX:+UseG1GC",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+ParallelRefProcEnabled",
    "-XX:+DisableExplicitGC",
    "-XX:G1PeriodicGCInterval=60000",
    "-XX:MinHeapFreeRatio=10",
    "-XX:MaxHeapFreeRatio=30",
];

/// The JVM options for a server with `memory_mb`: the heap, then the tuned flags. `launch` puts
/// these first; [`use_flags`] swaps the tuned flags for a server's own.
pub fn jvm(memory_mb: i64) -> Vec<String> {
    let mut args = vec![
        format!("-Xms{}M", START_HEAP_MB.min(memory_mb)),
        format!("-Xmx{memory_mb}M"),
    ];
    args.extend(TUNED_FLAGS.map(String::from));
    args
}

/// Replace the tuned flags [`jvm`] put in `args` with `flags`.
pub fn use_flags(args: &mut Vec<String>, flags: Vec<String>) {
    args.splice(2..2 + TUNED_FLAGS.len(), flags);
}

/// A server's own flags, `None` for the tuned ones.
pub async fn flags(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Option<Vec<String>>> {
    let flags: Option<String> =
        sqlx::query_scalar("SELECT flags FROM jvm_flags WHERE server_id = ?")
            .bind(server_id)
            .fetch_optional(db)
            .await?;
    Ok(flags.and_then(|json| serde_json::from_str(&json).ok()))
}

/// Give a server its own flags, or `None` to go back to the tuned ones.
pub async fn set_flags(
    db: &SqlitePool,
    server_id: &str,
    flags: Option<&[String]>,
) -> sqlx::Result<()> {
    match flags {
        Some(flags) => {
            sqlx::query(
                "INSERT INTO jvm_flags (server_id, flags) VALUES (?, ?) \
                 ON CONFLICT (server_id) DO UPDATE SET flags = excluded.flags",
            )
            .bind(server_id)
            .bind(serde_json::to_string(flags).expect("serializes"))
            .execute(db)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM jvm_flags WHERE server_id = ?")
                .bind(server_id)
                .execute(db)
                .await?;
        }
    }
    Ok(())
}

/// The memory every server together may use, in MB.
pub async fn assigned_mb(db: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT coalesce(sum(memory_mb), 0) FROM servers")
        .fetch_one(db)
        .await
}

/// This machine's memory in MB, `None` when the OS does not say.
pub fn machine_mb() -> Option<u64> {
    platform::machine_bytes().map(|bytes| bytes / (1024 * 1024))
}

#[cfg(target_os = "linux")]
mod platform {
    pub fn machine_bytes() -> Option<u64> {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = info.lines().find(|line| line.starts_with("MemTotal:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    pub fn machine_bytes() -> Option<u64> {
        let mut bytes: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        // SAFETY: `hw.memsize` is a u64, and `bytes` and `size` describe it.
        let result = unsafe {
            libc::sysctlbyname(
                c"hw.memsize".as_ptr(),
                (&raw mut bytes).cast(),
                &raw mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        (result == 0).then_some(bytes)
    }
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::System::SystemInformation::{
        GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };

    pub fn machine_bytes() -> Option<u64> {
        let mut status = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `status` is a MEMORYSTATUSEX with its length set.
        (unsafe { GlobalMemoryStatusEx(&raw mut status) } != 0)
            .then_some(status.ullTotalPhys)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    pub fn machine_bytes() -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, servers::records};

    #[test]
    fn the_heap_starts_small_and_custom_flags_replace_the_tuned_ones() {
        let args = jvm(4096);
        assert_eq!(args[..2], ["-Xms512M", "-Xmx4096M"]);
        assert_eq!(args[2..], TUNED_FLAGS);
        assert_eq!(jvm(256)[0], "-Xms256M");
        let mut args = jvm(2048);
        args.push("-Djava.io.tmpdir=tmp".into());
        use_flags(&mut args, vec!["-XX:+UseZGC".into()]);
        assert_eq!(
            args,
            [
                "-Xms512M",
                "-Xmx2048M",
                "-XX:+UseZGC",
                "-Djava.io.tmpdir=tmp"
            ]
        );
    }

    #[tokio::test]
    async fn flags_are_set_and_reset() {
        let db = db::memory().await;
        let server =
            records::create(&db, "A", |_| true).await.unwrap().unwrap();
        assert_eq!(flags(&db, &server.id).await.unwrap(), None);
        let own = vec!["-XX:+UseZGC".to_owned()];
        set_flags(&db, &server.id, Some(&own)).await.unwrap();
        assert_eq!(flags(&db, &server.id).await.unwrap(), Some(own));
        set_flags(&db, &server.id, None).await.unwrap();
        assert_eq!(flags(&db, &server.id).await.unwrap(), None);
    }

    #[test]
    fn this_machine_reports_its_memory() {
        assert!(machine_mb().is_some_and(|mb| mb > 256));
    }
}
