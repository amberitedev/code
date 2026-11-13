use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};
use uuid::Uuid;

use super::api_error;
use crate::{
    Engine,
    access::{
        Access, Caller, Edit, EngineAccess, InstanceAccess, Owner, Permission,
        View,
    },
    endpoints::install::{
        AcceptPendingSharedInstanceInvite, AcceptSharedInstanceInvite,
        CancelInstallJob, DeleteSharedInstanceVersion, DismissInstallJob,
        DuplicateInstance, GetInstallJob, GetInstallJobSupportDetails,
        GetSharedHistorySettings, GetSharedInstanceHistory,
        GetSharedInstanceInstallPreview, GetSharedInstanceInvitePreview,
        GetSharedInstanceUpdatePreview, InstallInstance, InstallSharedInstance,
        ListInstallJobs, PauseInstallJob, RestoreSharedInstanceVersion,
        ResumeInstallJob, RetryInstallJob, SetSharedHistoryKept,
        UpdateInstanceContent, UpdateSharedInstance,
    },
    install::{self, InstallJobSnapshot, model::InstallTarget},
    instance::{
        self, SharedHistorySettings, SharedInstanceHistoryEntry,
        SharedInstanceInstallPreview, SharedInstanceInviteInstallPreview,
        SharedInstanceUpdatePreview,
    },
};

type JobResponse = Result<Json<InstallJobSnapshot>, ApiError>;

pub async fn install_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<InstallInstance>,
) -> JobResponse {
    install::install_existing_instance(instance.id, request.body.force, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn duplicate_instance(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<DuplicateInstance>,
) -> JobResponse {
    install::duplicate_instance(request.params.id, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn update_instance_content(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<UpdateInstanceContent>,
) -> JobResponse {
    install::bulk_update_content(instance.id, request.body, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_shared_instance_update_preview(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    _: Req<GetSharedInstanceUpdatePreview>,
) -> Result<Json<Option<SharedInstanceUpdatePreview>>, ApiError> {
    instance::get_shared_instance_update_preview(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn update_shared_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<UpdateSharedInstance>,
) -> JobResponse {
    instance::update_shared_instance(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_shared_instance_history(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<GetSharedInstanceHistory>,
) -> Result<Json<Vec<SharedInstanceHistoryEntry>>, ApiError> {
    instance::get_shared_instance_history(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn restore_shared_instance_version(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    request: Req<RestoreSharedInstanceVersion>,
) -> JobResponse {
    instance::restore_shared_instance_version(
        &engine,
        &instance.id,
        request.params.version,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn delete_shared_instance_version(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    request: Req<DeleteSharedInstanceVersion>,
) -> Result<Empty, ApiError> {
    instance::delete_shared_instance_version(
        &engine,
        &instance.id,
        request.params.version,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn get_shared_instance_install_preview(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetSharedInstanceInstallPreview>,
) -> Result<Json<SharedInstanceInstallPreview>, ApiError> {
    instance::get_shared_instance_install_preview(
        &engine,
        &request.params.id,
        request.query.name,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn install_shared_instance(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<InstallSharedInstance>,
) -> JobResponse {
    let install = request.body;
    instance::install_shared_instance(
        &engine,
        &request.params.id,
        install.name,
        install.manager_id,
        install.server_manager_name,
        install.server_manager_icon_url,
        install.instance_icon_url,
        install.instance_icon_config,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn accept_shared_instance_invite(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<AcceptSharedInstanceInvite>,
) -> Result<Empty, ApiError> {
    instance::accept_shared_instance_invite_for_install(
        &engine,
        &request.params.id,
        &request.params.invite_id,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn accept_pending_shared_instance_invite(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<AcceptPendingSharedInstanceInvite>,
) -> Result<Empty, ApiError> {
    instance::accept_pending_shared_instance_invite(&engine, &request.params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn get_shared_instance_invite_preview(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetSharedInstanceInvitePreview>,
) -> Result<Json<SharedInstanceInviteInstallPreview>, ApiError> {
    instance::get_shared_instance_invite_install_preview(
        &engine,
        &request.params.id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_shared_history_settings(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<GetSharedHistorySettings>,
) -> Result<Json<SharedHistorySettings>, ApiError> {
    instance::get_shared_history_settings(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn set_shared_history_kept(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<SetSharedHistoryKept>,
) -> Result<Json<SharedHistorySettings>, ApiError> {
    instance::set_shared_history_kept(&engine, request.body.kept)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn list_install_jobs(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<ListInstallJobs>,
) -> Result<Json<Vec<InstallJobSnapshot>>, ApiError> {
    let jobs = install::list_jobs(request.query.include_finished, &engine)
        .await
        .map_err(api_error)?;
    let mut visible = Vec::with_capacity(jobs.len());
    for job in jobs {
        if require_job(&*access, &caller, &job, Permission::View)
            .await
            .is_ok()
        {
            visible.push(job);
        }
    }
    Ok(Json(visible))
}

pub async fn get_install_job(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<GetInstallJob>,
) -> JobResponse {
    job(
        &engine,
        &*access,
        &caller,
        request.params.id,
        Permission::View,
    )
    .await
    .map(Json)
}

pub async fn retry_install_job(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<RetryInstallJob>,
) -> JobResponse {
    let id = request.params.id;
    job(&engine, &*access, &caller, id, Permission::Edit).await?;
    install::retry_job(id, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn pause_install_job(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<PauseInstallJob>,
) -> JobResponse {
    let id = request.params.id;
    job(&engine, &*access, &caller, id, Permission::Edit).await?;
    install::pause_job(id, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn resume_install_job(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<ResumeInstallJob>,
) -> JobResponse {
    let id = request.params.id;
    job(&engine, &*access, &caller, id, Permission::Edit).await?;
    install::resume_job(id, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn cancel_install_job(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<CancelInstallJob>,
) -> JobResponse {
    let id = request.params.id;
    job(&engine, &*access, &caller, id, Permission::Edit).await?;
    install::cancel_job(id, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn dismiss_install_job(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<DismissInstallJob>,
) -> Result<Empty, ApiError> {
    let id = request.params.id;
    job(&engine, &*access, &caller, id, Permission::Edit).await?;
    install::dismiss_job(id, &engine)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn get_install_job_support_details(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<GetInstallJobSupportDetails>,
) -> Result<Json<String>, ApiError> {
    let id = request.params.id;
    job(&engine, &*access, &caller, id, Permission::View).await?;
    install::job_support_details(id, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn job(
    engine: &Engine,
    access: &dyn Access,
    caller: &Caller,
    id: Uuid,
    permission: Permission,
) -> Result<InstallJobSnapshot, ApiError> {
    let job = install::get_job(id, engine).await.map_err(api_error)?;
    require_job(access, caller, &job, permission).await?;
    Ok(job)
}

/// A job is its instance's; one that has not made its instance yet needs the whole engine.
async fn require_job(
    access: &dyn Access,
    caller: &Caller,
    job: &InstallJobSnapshot,
    permission: Permission,
) -> Result<(), ApiError> {
    let instance = job.instance_id.as_deref().or(match &job.target {
        InstallTarget::ExistingInstance { instance_id } => Some(instance_id),
        InstallTarget::NewInstance { instance_id } => instance_id.as_deref(),
    });
    let permission = if instance.is_some() {
        permission
    } else {
        Permission::Owner
    };
    access
        .require(caller, instance, permission)
        .await
        .map(|_| ())
}
