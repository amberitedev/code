//! API for interacting with Theseus
pub mod cache;
pub mod debug_info;
pub mod instance;
pub mod jre;
pub mod logs;
pub mod metadata;
pub mod pack;
pub mod process;
pub mod server_address;
pub mod server_list;
pub mod settings;
pub mod tags;
pub mod upload;
pub mod worlds;

pub mod data {
    pub use crate::state::{
        AppliedContentSetPatch, CacheBehaviour, CacheValueType, CachedEntry,
        CachedSharedInstanceSummary, ContentFile, ContentItem,
        ContentItemOwner, ContentItemProject, ContentItemVersion,
        CreateInstance, Dependency, DirectoryInfo, EditInstance, Hooks,
        IconBackground, IconConfig, InstanceInstallCandidate,
        InstanceInstallTarget, InstanceLaunchOverridesPatch, InstanceLink,
        InstanceMetadata, InstanceSyncedOption, InstanceSyncedOptions,
        InstanceTabVisibility, JavaVersion, LinkedModpackInfo, MemorySettings,
        ModLoader, Organization, OwnerType, ProcessMetadata, Project,
        ProjectType, ProjectV3, SearchResult, SearchResults, SearchResultsV3,
        SharedInstanceAttachment, SharedInstanceRole, TeamMember, User,
        Version, WindowSize,
    };
    pub use modrinth_content_management::{
        ContentType, ResolutionPreferences, ResolveContentPlan,
        ResolveContentRequest,
    };
}

pub mod prelude {
    pub use crate::{
        Engine,
        data::*,
        install, instance, jre, metadata, pack, process, settings,
        state::ReleaseChannel,
        util::{
            io::{IOError, canonicalize},
            network::tcp_listen_any_loopback,
        },
    };
}
