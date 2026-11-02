//! The update channel rule, as app-lib applies it (`state/cache.rs`, `ReleaseChannel`): a
//! channel takes its own version types and the more stable ones; when nothing there fits, the
//! next less stable type is tried, one step at a time. The channel content updates on is the
//! least stable of the one preferred and the one the installed version is on, so a beta
//! someone installed keeps getting betas.

use modrinth_content_management::VersionType;

use crate::content::model::modrinth::Version;

/// How a version type is named in Modrinth's API and in a stored update channel.
pub fn version_type_name(version_type: VersionType) -> &'static str {
    match version_type {
        VersionType::Release => "release",
        VersionType::Beta => "beta",
        VersionType::Alpha => "alpha",
    }
}

/// The version type named `name`, as [`version_type_name`] writes it.
pub fn parse_version_type(name: &str) -> Option<VersionType> {
    VersionType::ALL
        .into_iter()
        .find(|version_type| version_type_name(*version_type) == name)
}

/// The less stable of `a` and `b`.
pub fn least_stable(a: VersionType, b: VersionType) -> VersionType {
    a.max(b)
}

/// The version types to try for `channel`, in order: the channel's own first, then each less
/// stable type on its own.
pub fn version_type_fallbacks(
    channel: VersionType,
) -> &'static [&'static [VersionType]] {
    use VersionType::*;
    match channel {
        Release => &[&[Release], &[Beta], &[Alpha]],
        Beta => &[&[Release, Beta], &[Alpha]],
        Alpha => &[&[Release, Beta, Alpha]],
    }
}

/// The version of a pack to offer as an update over `installed`, from all of the project's
/// `versions`, as app-lib's `check_modpack_update` (`list_content.rs`) picks it: on the least
/// stable of `preferred` and the installed version's type, the first fallback step that has any
/// version decides, and offers its newest version published after the installed one, or nothing.
/// Minecraft and loader are not checked; installing the pick guards a change of either.
pub fn pack_update<'a>(
    installed: &Version,
    versions: &'a [Version],
    preferred: VersionType,
) -> Option<&'a Version> {
    let channel = least_stable(preferred, installed.version_type);
    let version_types =
        version_type_fallbacks(channel)
            .iter()
            .find(|version_types| {
                versions.iter().any(|version| {
                    version_types.contains(&version.version_type)
                })
            })?;
    versions
        .iter()
        .filter(|version| {
            version.id != installed.id
                && version.date_published > installed.date_published
                && version_types.contains(&version.version_type)
        })
        .max_by_key(|version| version.date_published)
}

#[cfg(test)]
mod tests {
    use super::*;
    use VersionType::*;

    fn version(id: &str, version_type: VersionType, day: u32) -> Version {
        Version {
            id: id.into(),
            project_id: "pack".into(),
            version_number: id.into(),
            version_type,
            loaders: Vec::new(),
            game_versions: Vec::new(),
            files: Vec::new(),
            dependencies: Vec::new(),
            date_published: chrono::NaiveDate::from_ymd_opt(2026, 1, day)
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|time| time.and_utc()),
            environment: None,
        }
    }

    #[test]
    fn a_pack_updates_to_its_newest_version_whatever_its_minecraft() {
        let mut installed = version("v1", Release, 1);
        installed.game_versions = vec!["1.21.1".into()];
        let mut newer = version("v2", Release, 9);
        newer.game_versions = vec!["1.21.4".into()];
        let versions = [installed.clone(), newer, version("v3", Release, 5)];
        let pick = pack_update(&installed, &versions, Release);
        assert_eq!(pick.map(|version| version.id.as_str()), Some("v2"));
    }

    #[test]
    fn a_pack_never_updates_to_an_older_version() {
        let installed = version("v2", Release, 9);
        let versions = [version("v1", Release, 1), installed.clone()];
        assert!(pack_update(&installed, &versions, Release).is_none());
    }

    #[test]
    fn the_first_step_with_any_version_decides_a_pack_update() {
        // Releases exist, so a newer beta is not offered on the release channel.
        let installed = version("v1", Release, 1);
        let versions = [installed.clone(), version("b1", Beta, 9)];
        assert!(pack_update(&installed, &versions, Release).is_none());
        // An installed beta widens the channel to betas.
        let installed = version("b1", Beta, 1);
        let versions = [
            installed.clone(),
            version("b2", Beta, 9),
            version("a1", Alpha, 10),
        ];
        let pick = pack_update(&installed, &versions, Release);
        assert_eq!(pick.map(|version| version.id.as_str()), Some("b2"));
    }

    #[test]
    fn version_types_read_back_by_name() {
        for version_type in VersionType::ALL {
            assert_eq!(
                parse_version_type(version_type_name(version_type)),
                Some(version_type)
            );
        }
        assert_eq!(parse_version_type("stable"), None);
    }

    #[test]
    fn the_installed_channel_widens_the_preferred_one() {
        assert_eq!(least_stable(Release, Beta), Beta);
        assert_eq!(least_stable(Alpha, Release), Alpha);
        assert_eq!(least_stable(Release, Release), Release);
    }

    #[test]
    fn a_channel_falls_back_one_step_at_a_time() {
        assert_eq!(
            version_type_fallbacks(Release),
            &[&[Release][..], &[Beta], &[Alpha]]
        );
        assert_eq!(
            version_type_fallbacks(Beta),
            &[&[Release, Beta][..], &[Alpha]]
        );
        assert_eq!(
            version_type_fallbacks(Alpha),
            &[&[Release, Beta, Alpha][..]]
        );
    }
}
