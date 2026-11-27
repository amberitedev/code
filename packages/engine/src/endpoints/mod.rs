//! The engine's Endpoints, over its own types. `crate::routes` serves them; their TypeScript goes
//! to `packages/contracts/ts/engine.ts` with the engine's other wire types.

/// Declares the route groups, and lists every route so no group escapes the tests.
macro_rules! groups {
    ($($group:ident),+ $(,)?) => {
        $(pub mod $group;)+

        /// Every engine route, by group.
        pub const ROUTES: &[&[contracts::Route]] = &[$($group::ROUTES),+];

        /// The engine routes whose types name a host path.
        #[cfg(feature = "export-ts")]
        pub fn host_paths() -> Vec<&'static str> {
            [$($group::host_paths()),+].concat()
        }
    };
}

groups!(
    cache, content, files, install, instances, jre, loading, logs, metadata,
    process, sharing, store, tags, worlds,
);

#[cfg(all(test, feature = "export-ts"))]
mod tests {
    use contracts::Reach;

    /// A Core serves its servers' `Shared` routes to remote users, so none may carry a path on
    /// the Core's disk.
    #[test]
    fn every_route_says_who_serves_it_and_no_shared_one_names_a_host_path() {
        let routes = || super::ROUTES.iter().copied().flatten();
        let unclassified: Vec<_> = routes()
            .filter(|route| route.reach.is_none())
            .map(|route| route.name)
            .collect();
        assert_eq!(unclassified, Vec::<&str>::new());
        let host_paths = super::host_paths();
        assert!(host_paths.contains(&"InstallJava"), "{host_paths:?}");
        let shared: Vec<_> = routes()
            .filter(|route| {
                route.reach == Some(Reach::Shared)
                    && host_paths.contains(&route.name)
            })
            .map(|route| route.name)
            .collect();
        assert_eq!(shared, Vec::<&str>::new());
    }
}
