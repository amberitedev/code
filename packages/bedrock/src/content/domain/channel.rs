//! The update channel rule, as app-lib applies it (`state/cache.rs`, `ReleaseChannel`): a
//! channel takes its own version types and the more stable ones; when nothing there fits, the
//! next less stable type is tried, one step at a time. The channel content updates on is the
//! least stable of the one preferred and the one the installed version is on, so a beta
//! someone installed keeps getting betas.

use modrinth_content_management::VersionType;

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

#[cfg(test)]
mod tests {
    use super::*;
    use VersionType::*;

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
