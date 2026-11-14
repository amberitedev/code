import { config } from '@/config'
import { apiClient } from '@/platform/modrinth-client'

export type ReportItemType = 'project' | 'version' | 'user' | 'shared-instance'

export interface CreateReportRequest {
	report_type: string
	item_id: string
	item_type: ReportItemType
	body: string
	uploaded_images: string[]
}

export interface CreateReportResponse {
	id: string
}

export async function create_report(request: CreateReportRequest): Promise<CreateReportResponse> {
	// The account backend does not take reports yet, and Modrinth has no record of its users or shares.
	if (config.accountApiUrl && ['user', 'shared-instance'].includes(request.item_type)) {
		throw new Error(
			'Reporting local accounts and shared instances is not supported by this service yet.',
		)
	}
	return await apiClient.labrinth.reports_v3.create(request)
}
