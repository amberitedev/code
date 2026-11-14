//! Every share and disk limit, defined once.

crate::consts! {
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
    /// The most a version's object list in a store may weigh: 4,096 objects take well under a
    /// megabyte.
    MAX_VERSION_LIST_BYTES: u64 = 4 * 1024 * 1024;
}
