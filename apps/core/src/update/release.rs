//! Copal releases: which one a channel resolves to, and the signed manifest that says what is in
//! it.
//!
//! A release is a GitHub release tagged `copal-v<version>` holding one binary per target, the
//! manifest [`MANIFEST`] listing every binary's SHA-256, and its minisign signature
//! [`SIGNATURE`]. The release workflow (`.github/workflows/copal-release.yml`) signs the manifest
//! with the key whose public half is [`PUBLIC_KEY`]. Every other release in the repository, such
//! as the Modrinth App's, is ignored.

use semver::Version;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The public half of the release signing key, in minisign's format. Its secret half is the
/// `COPAL_MINISIGN_KEY` repository secret.
pub const PUBLIC_KEY: &str =
    "RWQqPj5ql/C7RJ/2nm3MHZkzq7zGJNkPz82ttxT3HnQokqfiRH2Uqzjb";

/// Tags of Copal releases start with this.
const TAG_PREFIX: &str = "copal-v";
pub const MANIFEST: &str = "copal-manifest.json";
pub const SIGNATURE: &str = "copal-manifest.json.minisig";

/// This build's target as release files name it, `None` on a platform Copal is not released
/// for.
pub const TARGET: Option<&str> = if cfg!(all(windows, target_arch = "x86_64")) {
    Some("windows-x64")
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    Some("macos-arm64")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("linux-x64")
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    Some("linux-arm64")
} else {
    None
};

/// A release channel. Each one also gets every release of the channels before it: beta gets
/// stable releases, alpha gets both.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    TS,
    sqlx::Type,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
    Alpha,
}

impl Channel {
    /// The channel `version` is released on: stable without a pre-release, beta for
    /// `-beta.N`, alpha for `-alpha.N`, and none for any other pre-release.
    pub fn of(version: &Version) -> Option<Self> {
        let pre = version.pre.as_str();
        if pre.is_empty() {
            Some(Self::Stable)
        } else if pre == "beta" || pre.starts_with("beta.") {
            Some(Self::Beta)
        } else if pre == "alpha" || pre.starts_with("alpha.") {
            Some(Self::Alpha)
        } else {
            None
        }
    }
}

/// One release in the GitHub releases list, with only what the updater reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

impl Release {
    /// The Copal version this release is, `None` for any other release.
    fn copal_version(&self) -> Option<Version> {
        if self.draft {
            return None;
        }
        Version::parse(self.tag_name.strip_prefix(TAG_PREFIX)?).ok()
    }

    /// Where to download the asset called `name`.
    pub fn asset(&self, name: &str) -> Option<&str> {
        self.assets
            .iter()
            .find(|asset| asset.name == name)
            .map(|asset| asset.browser_download_url.as_str())
    }
}

/// The newest Copal release `channel` gets, newer or not than the running version.
pub fn resolve(
    releases: &[Release],
    channel: Channel,
) -> Option<(Version, &Release)> {
    releases
        .iter()
        .filter_map(|release| Some((release.copal_version()?, release)))
        .filter(|(version, _)| {
            Channel::of(version).is_some_and(|of| of <= channel)
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
}

/// What a release holds, as the release workflow signed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: Version,
    pub channel: Channel,
    pub files: Vec<File>,
}

/// One binary of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    /// See [`TARGET`].
    pub target: String,
    /// The release asset's name.
    pub name: String,
    /// Lowercase hex.
    pub sha256: String,
    pub size: u64,
}

/// Why a release cannot be installed.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error(
        "the release manifest's signature is not valid, so the release is not trusted"
    )]
    BadSignature,
    #[error("the release manifest is unreadable: {0}")]
    Invalid(String),
    #[error(
        "the release manifest is for {found}, but the release is tagged {expected}"
    )]
    WrongVersion { expected: Version, found: Version },
    #[error("the release manifest puts {version} on the wrong channel")]
    WrongChannel { version: Version },
    #[error("this release has no Copal for {0}")]
    NoTarget(String),
    #[error("Copal is not released for this platform")]
    UnsupportedPlatform,
}

impl Manifest {
    /// Read a manifest whose minisign `signature` checks out against `public_key`. The
    /// signature covers every byte, so nothing in an unsigned or altered manifest is trusted.
    pub fn verify(
        bytes: &[u8],
        signature: &str,
        public_key: &str,
    ) -> Result<Self, ManifestError> {
        let key = minisign_verify::PublicKey::from_base64(public_key)
            .map_err(|error| ManifestError::Invalid(error.to_string()))?;
        let signature = minisign_verify::Signature::decode(signature)
            .map_err(|_| ManifestError::BadSignature)?;
        key.verify(bytes, &signature, false)
            .map_err(|_| ManifestError::BadSignature)?;
        serde_json::from_slice(bytes)
            .map_err(|error| ManifestError::Invalid(error.to_string()))
    }

    /// The binary for `target`, once the manifest is checked against the release `tagged`, so a
    /// validly signed manifest cannot be passed off as another release.
    pub fn file(
        &self,
        tagged: &Version,
        target: Option<&str>,
    ) -> Result<&File, ManifestError> {
        if &self.version != tagged {
            return Err(ManifestError::WrongVersion {
                expected: tagged.clone(),
                found: self.version.clone(),
            });
        }
        if Channel::of(&self.version) != Some(self.channel) {
            return Err(ManifestError::WrongChannel {
                version: self.version.clone(),
            });
        }
        let target = target.ok_or(ManifestError::UnsupportedPlatform)?;
        self.files
            .iter()
            .find(|file| file.target == target)
            .ok_or_else(|| ManifestError::NoTarget(target.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signed with `minisign -S` by a test key, the way the release workflow signs.
    const SIGNED: &str = r#"{"version":"0.2.0","channel":"stable","files":[{"target":"windows-x64","name":"copal-windows-x64.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000001","size":1},{"target":"macos-arm64","name":"copal-macos-arm64","sha256":"0000000000000000000000000000000000000000000000000000000000000002","size":2},{"target":"linux-x64","name":"copal-linux-x64","sha256":"0000000000000000000000000000000000000000000000000000000000000003","size":3},{"target":"linux-arm64","name":"copal-linux-arm64","sha256":"0000000000000000000000000000000000000000000000000000000000000004","size":4}]}"#;
    const SIGNED_SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RUR8qp1Hpo5PbVqzaf0xb+0mJ6fDM7IcWlp+3ax+A1Y6cIZtHnVonyzxptUEwk+6hsUHqBZsQ+0EAp4KBMdOTQFderqn+hoMqAI=
trusted comment: copal 0.2.0 test manifest
mINo/gYrYpZyaxCbxH2qcY0JDcSIxwE8hmuhaSlbetKaT2+5+icaNVkLEbo2lc1QAJ/XV0iNIK9wQrlRoiRjDQ==
";
    const TEST_KEY: &str =
        "RWR8qp1Hpo5PbTJ+emVLiU6eLbGCz2C+0TOjxxoojaSw94gDx/8rBwhN";

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            assets: Vec::new(),
        }
    }

    #[test]
    fn a_signed_manifest_is_read() {
        let manifest =
            Manifest::verify(SIGNED.as_bytes(), SIGNED_SIGNATURE, TEST_KEY)
                .unwrap();
        assert_eq!(manifest.version, version("0.2.0"));
        assert_eq!(manifest.channel, Channel::Stable);
        assert_eq!(manifest.files.len(), 4);
    }

    #[test]
    fn an_altered_manifest_or_another_key_is_refused() {
        let altered = SIGNED.replace("0.2.0", "0.9.0");
        assert!(matches!(
            Manifest::verify(altered.as_bytes(), SIGNED_SIGNATURE, TEST_KEY),
            Err(ManifestError::BadSignature)
        ));
        assert!(matches!(
            Manifest::verify(SIGNED.as_bytes(), SIGNED_SIGNATURE, PUBLIC_KEY),
            Err(ManifestError::BadSignature)
        ));
        assert!(matches!(
            Manifest::verify(SIGNED.as_bytes(), "not a signature", TEST_KEY),
            Err(ManifestError::BadSignature)
        ));
    }

    #[test]
    fn the_file_is_the_one_for_this_target_of_the_tagged_release() {
        let manifest =
            Manifest::verify(SIGNED.as_bytes(), SIGNED_SIGNATURE, TEST_KEY)
                .unwrap();
        let tagged = version("0.2.0");
        for target in ["windows-x64", "macos-arm64", "linux-x64", "linux-arm64"]
        {
            let file = manifest.file(&tagged, Some(target)).unwrap();
            assert_eq!(file.target, target);
        }
        assert!(matches!(
            manifest.file(&tagged, Some("freebsd-x64")),
            Err(ManifestError::NoTarget(_))
        ));
        assert!(matches!(
            manifest.file(&tagged, None),
            Err(ManifestError::UnsupportedPlatform)
        ));
        // A validly signed manifest of an older release, served under a newer tag.
        assert!(matches!(
            manifest.file(&version("0.3.0"), Some("linux-x64")),
            Err(ManifestError::WrongVersion { .. })
        ));
        let mislabeled = Manifest {
            channel: Channel::Alpha,
            ..manifest
        };
        assert!(matches!(
            mislabeled.file(&tagged, Some("linux-x64")),
            Err(ManifestError::WrongChannel { .. })
        ));
    }

    #[test]
    fn channels_resolve_to_copal_releases_only() {
        let releases = [
            release("v0.21.10"),
            release("v9.0.0"),
            release("copal-v0.2.0"),
            release("copal-v0.3.0-beta.1"),
            release("copal-v0.4.0-alpha.2"),
            release("copal-v0.5.0-rc.1"),
            release("copal-vbroken"),
            Release {
                draft: true,
                ..release("copal-v1.0.0")
            },
        ];
        let picked = |channel| {
            resolve(&releases, channel).map(|(version, _)| version.to_string())
        };
        assert_eq!(picked(Channel::Stable).as_deref(), Some("0.2.0"));
        assert_eq!(picked(Channel::Beta).as_deref(), Some("0.3.0-beta.1"));
        assert_eq!(picked(Channel::Alpha).as_deref(), Some("0.4.0-alpha.2"));
        assert!(resolve(&[release("v0.21.10")], Channel::Alpha).is_none());
    }

    #[test]
    fn a_newer_stable_release_beats_an_older_beta() {
        let releases =
            [release("copal-v0.3.0-beta.1"), release("copal-v0.3.0")];
        let (version, _) = resolve(&releases, Channel::Beta).unwrap();
        assert_eq!(version, self::version("0.3.0"));
    }
}
