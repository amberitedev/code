//! Every share and disk limit, defined once. The App's TypeScript reads the same numbers from
//! `packages/api-client/src/modules/bedrock/limits.ts`, generated from this file.
//!
//! `cargo test -p bedrock` fails when that file differs from what this one generates. Regenerate
//! it with `cargo test -p bedrock -- --ignored write_typescript`.

/// Declares each limit as a `pub const` and, for the tests, as a TypeScript `export const`.
macro_rules! limits {
    ($($(#[doc = $doc:literal])+ $name:ident: $ty:ty = $value:expr;)+) => {
        $($(#[doc = $doc])+ pub const $name: $ty = $value;)+

        #[cfg(test)]
        fn declarations() -> String {
            let mut out = String::new();
            $(
                out.push_str("\n/**\n");
                for line in [$($doc),+] {
                    out.push_str(" *");
                    out.push_str(line.trim_end());
                    out.push('\n');
                }
                out.push_str(" */\n");
                out.push_str(&format!("export const {} = {}\n", stringify!($name), $name));
            )+
            out
        }
    };
}

limits! {
    /// The most plain bytes one share version may hold. The App checks it before it uploads
    /// anything; the backend enforces it. Placeholder, decide later.
    MAX_SHARE_BYTES: u64 = 1024 * 1024 * 1024;
    /// What every account may keep on other people's Cores. The backend enforces it.
    /// Placeholder, decide later.
    ALLOWANCE_BYTES: u64 = 3 * 1024 * 1024 * 1024;
    /// The most entries an archive a Core unpacks (a pack, a share, an upload) may have.
    MAX_ARCHIVE_ENTRIES: usize = 200_000;
    /// The most an archive entry may grow when unpacked, as uncompressed bytes per compressed
    /// byte. Past this it is a zip bomb.
    MAX_COMPRESSION_RATIO: u64 = 100;
    /// The free disk space a Core keeps after anything it writes: what it writes must fit in the
    /// free space less this.
    DISK_MARGIN_BYTES: u64 = 256 * 1024 * 1024;
}

#[cfg(test)]
mod typescript {
    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../api-client/src/modules/bedrock/limits.ts"
    );

    fn file() -> String {
        format!(
            "// Generated from packages/bedrock/src/limits.rs. Do not edit.\n\
             // Regenerate: cargo test -p bedrock -- --ignored write_typescript\n{}",
            super::declarations()
        )
    }

    #[test]
    fn typescript_is_up_to_date() {
        let written = std::fs::read_to_string(PATH).unwrap_or_default();
        assert!(
            written.replace("\r\n", "\n") == file(),
            "{PATH} is out of date. Run: cargo test -p bedrock -- --ignored write_typescript"
        );
    }

    #[test]
    #[ignore = "writes the generated file; run on purpose"]
    fn write_typescript() {
        std::fs::create_dir_all(std::path::Path::new(PATH).parent().unwrap())
            .unwrap();
        std::fs::write(PATH, file()).unwrap();
    }
}
