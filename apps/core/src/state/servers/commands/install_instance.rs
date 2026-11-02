//! Applying the latest version of a share to a server that follows an App instance. The route
//! (`api::content::install`) reads the version first, so one that cannot go fails the request.
//! A stopped server installs it now. A running one keeps running: Core fetches the version's
//! files, then the version waits for the server to stop, or with `restart` Core stops the
//! server, and it applies then (see [`super::apply_pending`]).

use std::sync::Arc;

use sharing_client::backend::Backend;
use sharing_client::consume::ReadyVersion;
use tokio::sync::Notify;

use super::{
    apply_pending::apply_pending,
    drop_own_share::drop_own_share,
    forget_pack::forget_pack,
    install_pack::{self as content_install, PackSource},
    write_shared_pack as shared,
};
use crate::{
    servers::ServerError,
    state::servers::{
        Busy, Content, Error, Installing, PendingPack, PropertiesFields,
        SharedInstanceLink, adapters::sqlite::content_rows,
    },
};

/// What to install: the instance the server follows, its share, and how.
pub struct InstanceInstall {
    pub instance_id: String,
    pub instance_name: String,
    pub shared_instance_id: String,
    pub properties: Option<PropertiesFields>,
    /// Stop a running server to apply the version now, and start it again after.
    pub restart: bool,
    /// Install a version for another Minecraft.
    pub force: bool,
}

/// Install `version`, read through `backend`, on server `id`, which `busy` holds for content
/// changes. `owed` wakes sync: following an instance drops the server's own share, and may drop
/// the players of the instance it followed before.
pub async fn install_instance(
    content: &Arc<Content>,
    owed: Arc<Notify>,
    id: &str,
    busy: Busy,
    backend: Backend,
    version: ReadyVersion,
    install: InstanceInstall,
) -> Result<(), Error> {
    let InstanceInstall {
        instance_id,
        instance_name,
        shared_instance_id,
        properties,
        restart,
        force,
    } = install;
    match content.servers.hold(id) {
        Ok(hold) => {
            // A version still waiting from an earlier push is superseded by this one.
            let waiting = content_rows::clear_pending(&content.db, id).await?;
            forget_pack(content, waiting).await;
            content.spawn(
                hold,
                busy,
                Installing::Modpack,
                move |content, hold, progress| async move {
                    let written = shared::write_pack(
                        &content,
                        &backend,
                        hold.id(),
                        version,
                        &instance_name,
                    )
                    .await;
                    let (sha1, version) = match written {
                        Ok(written) => written,
                        Err(error) => {
                            content_rows::set_pack_error(
                                &content.db,
                                hold.id(),
                                Some(&format!("{error:#}")),
                            )
                            .await?;
                            return Err(error);
                        }
                    };
                    let source = PackSource::Stored {
                        sha1,
                        instance: Some(SharedInstanceLink {
                            instance_id,
                            instance_name,
                            shared_instance_id,
                            version: version.into(),
                        }),
                    };
                    let installed = content_install::pack(
                        &content, &hold, &progress, source, properties, force,
                    )
                    .await;
                    // Another instance may have dropped the players of the one it
                    // followed, even when a later step failed.
                    owed.notify_one();
                    let change = installed?;
                    drop_own_share(&content.db, &owed, hold.id()).await?;
                    Ok(change)
                },
            );
        }
        Err(ServerError::AlreadyRunning) => {
            let content = Arc::clone(content);
            let id = id.to_owned();
            tokio::spawn(async move {
                let written = shared::write_pack(
                    &content,
                    &backend,
                    &id,
                    version,
                    &instance_name,
                )
                .await;
                drop(busy);
                let result = async {
                    let (sha1, version) = written?;
                    let pending = PendingPack {
                        sha1,
                        instance_id,
                        instance_name,
                        shared_instance_id,
                        shared_version: version.into(),
                        start_after: restart,
                        force,
                    };
                    let replaced =
                        content_rows::set_pending(&content.db, &id, &pending)
                            .await?;
                    forget_pack(&content, replaced).await;
                    drop_own_share(&content.db, &owed, &id).await?;
                    if restart {
                        match content.servers.stop(&id).await {
                            // It stopped on its own while the version was fetched.
                            Err(ServerError::NotRunning) => {}
                            result => result?,
                        }
                    }
                    Ok::<_, Error>(())
                }
                .await;
                match result {
                    // The server may have stopped while the version was fetched.
                    Ok(()) => apply_pending(content, owed, id).await,
                    Err(error) => {
                        tracing::warn!(server = %id, %error, "could not take a pushed version");
                        let _ = content_rows::set_pack_error(
                            &content.db,
                            &id,
                            Some(&format!("{error:#}")),
                        )
                        .await;
                    }
                }
            });
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
