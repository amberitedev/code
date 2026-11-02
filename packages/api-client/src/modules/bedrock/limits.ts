// Generated from packages/bedrock/src/limits.rs. Do not edit.
// Regenerate: cargo test -p bedrock -- --ignored write_typescript

/**
 * The most plain bytes one share version may hold. The App checks it before it uploads
 * anything; the backend enforces it. Placeholder, decide later.
 */
export const MAX_SHARE_BYTES = 1073741824

/**
 * What every account may keep on other people's Cores. The backend enforces it.
 * Placeholder, decide later.
 */
export const ALLOWANCE_BYTES = 3221225472

/**
 * The most entries an archive a Core unpacks (a pack, a share, an upload) may have.
 */
export const MAX_ARCHIVE_ENTRIES = 200000

/**
 * The most an archive entry may grow when unpacked, as uncompressed bytes per compressed
 * byte. Past this it is a zip bomb.
 */
export const MAX_COMPRESSION_RATIO = 100

/**
 * The free disk space a Core keeps after anything it writes: what it writes must fit in the
 * free space less this.
 */
export const DISK_MARGIN_BYTES = 268435456

/**
 * The most a version's object list in a store may weigh: 4,096 objects take well under a
 * megabyte.
 */
export const MAX_VERSION_LIST_BYTES = 4194304
