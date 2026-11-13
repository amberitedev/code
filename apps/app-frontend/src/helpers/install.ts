import {
	type CreatePackLocation,
	type IconConfig,
	type ImportLauncherType,
	type InstallCreateInstanceRequest,
	type InstallErrorView,
	type InstallJavaStep,
	type InstallJobSnapshot,
	type InstallJobStatus,
	type InstallModpackPreview,
	type InstallPhaseId,
	type InstallPostInstallEdit,
	type InstallProgress,
	type InstallProgressSecondary,
	installs,
	launcherImport,
	localInstalls,
	type SharedHistorySettings,
	type SharedInstanceExternalFilePreview,
	type SharedInstanceHistoryEntry,
	type SharedInstanceInstallPreview,
	type SharedInstanceInviteInstallPreview,
	type SharedInstanceUpdateDiff,
	type SharedInstanceUpdatePreview,
} from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'
import type { AppEvents } from '@/providers/app-events'

export type {
	CreatePackLocation,
	InstallCreateInstanceRequest,
	InstallErrorView,
	InstallJavaStep,
	InstallJobSnapshot,
	InstallJobStatus,
	InstallModpackPreview,
	InstallPhaseId,
	InstallPostInstallEdit,
	InstallProgress,
	InstallProgressSecondary,
	SharedHistorySettings,
	SharedInstanceExternalFilePreview,
	SharedInstanceHistoryEntry,
	SharedInstanceInstallPreview,
	SharedInstanceInviteInstallPreview,
	SharedInstanceUpdateDiff,
	SharedInstanceUpdatePreview,
}

export type SharedInstanceUnavailableReason = 'deleted' | 'access_revoked' | 'quarantined'

export type PackLocationVersionId = Extract<CreatePackLocation, { type: 'fromVersionId' }>
export type PackLocationFile = Extract<CreatePackLocation, { type: 'fromFile' }>

const SHARED_INSTANCE_UNAVAILABLE_ERROR_CODES: Record<string, SharedInstanceUnavailableReason> = {
	shared_instance_deleted: 'deleted',
	shared_instance_access_revoked: 'access_revoked',
	shared_instance_quarantined: 'quarantined',
}
export const SHARED_INSTANCES_API_ERROR_CODE = 'shared_instances_api_error'
export const SHARED_INSTANCE_FILE_NOT_ALLOWED_ERROR_CODE = 'shared_instance_file_not_allowed'
export const SHARED_INSTANCE_OWNER_BEHIND_ERROR_CODE = 'shared_instance_owner_behind'
export const SHARED_INSTANCE_LIMIT_REACHED_ERROR_CODE = 'shared_instance_limit_reached'

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null
}

export function isSharedInstanceUnavailableError(error: unknown) {
	return getSharedInstanceUnavailableReason(error) !== null
}

export function isSharedInstancesApiError(error: unknown) {
	return isRecord(error) && error.code === SHARED_INSTANCES_API_ERROR_CODE
}

export function isSharedInstanceFileNotAllowedError(error: unknown) {
	return isRecord(error) && error.code === SHARED_INSTANCE_FILE_NOT_ALLOWED_ERROR_CODE
}

export function isSharedInstanceOwnerBehindError(error: unknown) {
	return isRecord(error) && error.code === SHARED_INSTANCE_OWNER_BEHIND_ERROR_CODE
}

export function isSharedInstanceLimitReachedError(error: unknown) {
	return isRecord(error) && error.code === SHARED_INSTANCE_LIMIT_REACHED_ERROR_CODE
}

export function getSharedInstanceUnavailableReason(
	error: unknown,
): SharedInstanceUnavailableReason | null {
	if (!isRecord(error) || typeof error.code !== 'string') return null
	return SHARED_INSTANCE_UNAVAILABLE_ERROR_CODES[error.code] ?? null
}

export function getErrorMessage(error: unknown): string {
	if (typeof error === 'string') return error
	if (error instanceof Error) return error.message || 'Unknown error'
	if (isRecord(error) && typeof error.message === 'string') return error.message
	return 'Unknown error'
}

export async function install_get_modpack_preview(location: CreatePackLocation) {
	return await apiClient.local.call(localInstalls.previewModpack, { body: location })
}

export async function install_create_instance(request: InstallCreateInstanceRequest) {
	return await apiClient.local.call(localInstalls.createInstance, { body: request })
}

export async function install_create_modpack_instance(
	location: CreatePackLocation,
	postInstallEdit?: InstallPostInstallEdit | null,
) {
	return await apiClient.local.call(localInstalls.createModpackInstance, {
		body: { location, post_install_edit: postInstallEdit },
	})
}

export async function install_get_shared_instance_preview(sharedInstanceId: string, name: string) {
	return await apiClient.local.call(installs.GetSharedInstanceInstallPreview, {
		params: { id: sharedInstanceId },
		query: { name },
	})
}

export async function install_get_shared_instance_invite_preview(inviteId: string) {
	return await apiClient.local.call(installs.GetSharedInstanceInvitePreview, {
		params: { id: inviteId },
	})
}

export async function install_accept_shared_instance_invite(
	sharedInstanceId: string,
	inviteId: string,
) {
	return await apiClient.local.call(installs.AcceptSharedInstanceInvite, {
		params: { id: sharedInstanceId, invite_id: inviteId },
	})
}

export async function install_accept_pending_shared_instance_invite(sharedInstanceId: string) {
	return await apiClient.local.call(installs.AcceptPendingSharedInstanceInvite, {
		params: { id: sharedInstanceId },
	})
}

export async function install_get_shared_instance_update_preview(instanceId: string) {
	return await apiClient.local.call(installs.GetSharedInstanceUpdatePreview, {
		params: { id: instanceId },
	})
}

export async function install_shared_instance(
	sharedInstanceId: string,
	name: string,
	managerId?: string | null,
	serverManagerName?: string | null,
	serverManagerIconUrl?: string | null,
	instanceIconUrl?: string | null,
	instanceIconConfig?: IconConfig | null,
) {
	return await apiClient.local.call(installs.InstallSharedInstance, {
		params: { id: sharedInstanceId },
		body: {
			name,
			manager_id: managerId ?? null,
			server_manager_name: serverManagerName ?? null,
			server_manager_icon_url: serverManagerIconUrl ?? null,
			instance_icon_url: instanceIconUrl ?? null,
			instance_icon_config: instanceIconConfig ?? null,
		},
	})
}

export async function install_update_shared_instance(instanceId: string) {
	return await apiClient.local.call(installs.UpdateSharedInstance, { params: { id: instanceId } })
}

/** The owner's history of the instance's share, newest first. Empty for a member. */
export async function install_get_shared_instance_history(instanceId: string) {
	return await apiClient.local.call(installs.GetSharedInstanceHistory, {
		params: { id: instanceId },
	})
}

/** Owner only. Puts the instance back to a version from its history, leaving it to push. */
export async function install_restore_shared_instance_version(instanceId: string, version: number) {
	return await apiClient.local.call(installs.RestoreSharedInstanceVersion, {
		params: { id: instanceId, version },
	})
}

/** Owner only. Deletes a version from the share's history. The latest one stays. */
export async function install_delete_shared_instance_version(instanceId: string, version: number) {
	return await apiClient.local.call(installs.DeleteSharedInstanceVersion, {
		params: { id: instanceId, version },
	})
}

export async function install_get_shared_history_settings() {
	return await apiClient.local.call(installs.GetSharedHistorySettings)
}

/** Keeps the newest `kept` versions of every share's history (all when null) and deletes the rest. */
export async function install_set_shared_history_kept(kept: number | null) {
	return await apiClient.local.call(installs.SetSharedHistoryKept, { body: { kept } })
}

export async function install_import_instance(
	launcherType: ImportLauncherType,
	basePath: string,
	instanceFolder: string,
) {
	return await apiClient.local.call(launcherImport.importInstance, {
		body: {
			launcher_type: launcherType,
			base_path: basePath,
			instance_folder: instanceFolder,
		},
	})
}

export async function install_duplicate_instance(sourceInstanceId: string) {
	return await apiClient.local.call(installs.DuplicateInstance, {
		params: { id: sourceInstanceId },
	})
}

export async function install_existing_instance(instanceId: string, force: boolean) {
	return await apiClient.local.call(installs.InstallInstance, {
		params: { id: instanceId },
		body: { force },
	})
}

export async function install_pack_to_existing_instance(
	instanceId: string,
	location: CreatePackLocation,
	postInstallEdit?: InstallPostInstallEdit | null,
) {
	return await apiClient.local.call(localInstalls.installPackToInstance, {
		params: { id: instanceId },
		body: { location, post_install_edit: postInstallEdit },
	})
}

export async function install_bulk_update_content(
	instanceId: string,
	updates: { project_path: string; version_id: string }[],
) {
	return await apiClient.local.call(installs.UpdateInstanceContent, {
		params: { id: instanceId },
		body: updates,
	})
}

export async function install_job_list(includeFinished: boolean) {
	return await apiClient.local.call(installs.ListInstallJobs, {
		query: { include_finished: includeFinished },
	})
}

export async function install_job_get(jobId: string) {
	return await apiClient.local.call(installs.GetInstallJob, { params: { id: jobId } })
}

export async function install_job_retry(jobId: string) {
	return await apiClient.local.call(installs.RetryInstallJob, { params: { id: jobId } })
}

export async function install_job_cancel(jobId: string) {
	return await apiClient.local.call(installs.CancelInstallJob, { params: { id: jobId } })
}

export async function install_job_pause(jobId: string) {
	return await apiClient.local.call(installs.PauseInstallJob, { params: { id: jobId } })
}

export async function install_job_resume(jobId: string) {
	return await apiClient.local.call(installs.ResumeInstallJob, { params: { id: jobId } })
}

export async function install_job_dismiss(jobId: string) {
	return await apiClient.local.call(installs.DismissInstallJob, { params: { id: jobId } })
}

export async function install_job_support_details(jobId: string) {
	return await apiClient.local.call(installs.GetInstallJobSupportDetails, {
		params: { id: jobId },
	})
}

export function installJobInstanceId(job: InstallJobSnapshot): string | null {
	return job.instance_id ?? job.target.instance_id ?? null
}

export function isInstallJobFinished(status: InstallJobStatus) {
	return (
		status === 'succeeded' ||
		status === 'failed' ||
		status === 'interrupted' ||
		status === 'canceled'
	)
}

function settleInstallJob(job: InstallJobSnapshot) {
	if (job.status === 'succeeded') return job

	if (job.error) throw job.error
	throw new Error(`Install job ${job.job_id} ${job.status}`)
}

export async function wait_for_install_job(events: AppEvents, jobId: string) {
	const current = await install_job_get(jobId)
	if (isInstallJobFinished(current.status)) return settleInstallJob(current)

	return await new Promise<InstallJobSnapshot>((resolve, reject) => {
		let finished = false
		let unlisten: (() => void) | null = null

		const cleanup = () => {
			if (unlisten) {
				unlisten()
				unlisten = null
			}
		}

		const resolveJob = (job: InstallJobSnapshot) => {
			if (finished || job.job_id !== jobId || !isInstallJobFinished(job.status)) return

			finished = true
			cleanup()

			try {
				resolve(settleInstallJob(job))
			} catch (err) {
				reject(err)
			}
		}

		const rejectWait = (err: unknown) => {
			if (finished) return
			finished = true
			cleanup()
			reject(err)
		}

		const fetchJob = () => install_job_get(jobId).then(resolveJob).catch(rejectWait)
		const unlistenJob = events.on('install_job', resolveJob)
		// A resync means the job's last event may be lost.
		const unlistenResync = events.on('resync', fetchJob)
		unlisten = () => {
			unlistenJob()
			unlistenResync()
		}
		void fetchJob()
	})
}
