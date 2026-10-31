import { AbstractModule } from '../../../core/abstract-module'
import { coreNodeUrl } from '../../../utils/node-url'
import type { Copal } from '../../copal/types'
import type { Archon } from '../types'

export class ArchonServersV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'archon_servers_v1'
	}

	/** Deletes a server on its Core, with its world, files, and backups. */
	public async deleteLocal(serverId: string): Promise<void> {
		if (!this.client.selfHostedHosting) throw new Error('Self-hosted hosting is not configured')
		await this.client.request<void>(`/servers/${serverId}`, {
			api: 'archon',
			version: 'local',
			method: 'DELETE',
			dangerous: true,
		})
	}

	/** Changes who sees a server linked to an instance, on its Core. */
	public async setLinkedInstance(
		serverId: string,
		settings: Archon.Servers.v0.LinkedInstanceSettings,
	): Promise<void> {
		await this.client.request<void>(`/servers/${serverId}/linked-instance`, {
			api: 'archon',
			version: 'local',
			method: 'PATCH',
			body: settings,
		})
	}

	// A Core's own routes are unversioned (`/servers/:id/...`), so `servers` goes in the version slot.

	/** A Core server's memory, JVM flags, and restart settings. */
	public async getPerformance(serverId: string): Promise<Copal.Performance> {
		return this.client.request<Copal.Performance>(`/${serverId}/performance`, {
			api: 'archon',
			version: 'servers',
			method: 'GET',
		})
	}

	/** Changes when a Core server restarts on its own. */
	public async setRestarts(serverId: string, restarts: Copal.Restarts): Promise<Copal.Performance> {
		return this.client.request<Copal.Performance>(`/${serverId}/restarts`, {
			api: 'archon',
			version: 'servers',
			method: 'PUT',
			body: restarts,
		})
	}

	/**
	 * Get list of servers for the authenticated user
	 * GET /v1/servers
	 */
	public async list(): Promise<Archon.Servers.v1.ServerFull[]> {
		const servers = await this.client.request<Archon.Servers.v1.ServerFull[]>('/servers', {
			api: 'archon',
			version: 1,
			method: 'GET',
		})
		return Promise.all(servers.map((server) => this.located(server)))
	}

	/**
	 * Get full server details including worlds, backups, and content
	 * GET /v1/servers/:server_id
	 */
	public async get(serverId: string): Promise<Archon.Servers.v1.ServerFull> {
		const server = await this.client.request<Archon.Servers.v1.ServerFull>(`/servers/${serverId}`, {
			api: 'archon',
			version: 1,
			method: 'GET',
		})
		return this.located(server)
	}

	/**
	 * A Core server's location is the Core itself, reached as this request was: its downloads go
	 * to `url_host`. See `coreNodeUrl`.
	 */
	private async located(
		server: Archon.Servers.v1.ServerFull,
	): Promise<Archon.Servers.v1.ServerFull> {
		if (!this.client.selfHostedHosting || server.location.status !== 'assigned') return server
		const base = await this.client.archon.cores_v1.hostingUrl(`/servers/${server.id}`)
		const location_metadata = {
			...server.location.location_metadata,
			url_host: coreNodeUrl(base, server.id),
		}
		return { ...server, location: { status: 'assigned', location_metadata } }
	}

	/**
	 * Select the best currently available download method for a world
	 * GET /v1/servers/:server_id/worlds/:world_id/select-download
	 */
	public async selectWorldDownload(
		serverId: string,
		worldId: string,
	): Promise<Archon.Servers.v1.WorldDownloadMethod> {
		return this.client.request<Archon.Servers.v1.WorldDownloadMethod>(
			`/servers/${serverId}/worlds/${worldId}/select-download`,
			{
				api: 'archon',
				version: 1,
				method: 'GET',
			},
		)
	}

	/**
	 * Get available regions
	 * GET /v1/regions
	 */
	public async getRegions(): Promise<Archon.Servers.v1.Region[]> {
		return this.client.request<Archon.Servers.v1.Region[]>('/regions', {
			api: 'archon',
			version: 1,
			method: 'GET',
			skipAuth: true,
		})
	}

	/**
	 * Get how many servers of each memory size fit in every public region
	 * GET /v1/stock
	 */
	public async getStock(memoryMb: number[]): Promise<Archon.Servers.v1.StockResponse> {
		return this.client.request<Archon.Servers.v1.StockResponse>('/stock', {
			api: 'archon',
			version: 1,
			method: 'GET',
			params: { memory_mb: memoryMb.join(',') },
			skipAuth: true,
		})
	}

	/**
	 * Get how many servers of each memory size fit in every public region when upgrading a server
	 * GET /v1/servers/:server_id/upgrade-stock
	 */
	public async getUpgradeStock(
		serverId: string,
		memoryMb: number[],
	): Promise<Archon.Servers.v1.StockResponse> {
		return this.client.request<Archon.Servers.v1.StockResponse>(
			`/servers/${serverId}/upgrade-stock`,
			{
				api: 'archon',
				version: 1,
				method: 'GET',
				params: { memory_mb: memoryMb.join(',') },
			},
		)
	}

	/**
	 * End the intro flow for a server
	 * DELETE /v1/servers/:id/flows/intro
	 */
	public async endIntro(serverId: string): Promise<void> {
		await this.client.request(`/servers/${serverId}/flows/intro`, {
			api: 'archon',
			version: 1,
			method: 'DELETE',
		})
	}

	/**
	 * Reset a world to onboarding
	 * POST /v1/servers/:id/worlds/:wid/onboard
	 */
	public async resetToOnboarding(serverId: string, worldId: string): Promise<void> {
		await this.client.request(`/servers/${serverId}/worlds/${worldId}/onboard`, {
			api: 'archon',
			version: 1,
			method: 'POST',
		})
	}

	/**
	 * Modify a world
	 * PATCH /v1/servers/:server_id/worlds/:world_id
	 */
	public async patchWorld(
		serverId: string,
		worldId: string,
		body: Archon.Servers.v1.PatchWorld,
	): Promise<void> {
		await this.client.request(`/servers/${serverId}/worlds/${worldId}`, {
			api: 'archon',
			version: 1,
			method: 'PATCH',
			body,
		})
	}

	/**
	 * Roll SFTP credentials for a server
	 * POST /v1/servers/:server_id/sftp/roll
	 */
	public async rollSftp(serverId: string): Promise<Archon.Servers.v1.SftpCredentials> {
		return this.client.request<Archon.Servers.v1.SftpCredentials>(
			`/servers/${serverId}/sftp/roll`,
			{
				api: 'archon',
				version: 1,
				method: 'POST',
			},
		)
	}
}
