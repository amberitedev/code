//! A server's memory and JVM flags. Java starts with a small heap, may grow it to the server's
//! memory, and with the tuned flags G1 hands unused memory back, so an idle server shrinks. The
//! tuned flags are the default; a server can have its own instead (`ServerRow::jvm_flags`), and
//! resetting goes back to the tuned ones.

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

/// This machine's memory in MB, by the engine's measure, `None` when the OS does not say.
pub async fn machine_mb() -> Option<u64> {
    let kib = engine::jre::get_max_memory().await.ok()?;
    (kib > 0).then_some(kib / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
