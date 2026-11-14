import {
	instanceGroups,
	type InstanceGroupDefinition,
	type InstanceGroupMembershipUpdate,
} from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

export type { InstanceGroupDefinition, InstanceGroupMembershipUpdate }

export const FAVORITES_GROUP_ID = 'group:favorites'
export const MAX_INSTANCE_GROUP_NAME_LENGTH = 256

export async function list_groups(): Promise<InstanceGroupDefinition[]> {
	return await apiClient.local.call(instanceGroups.ListInstanceGroups)
}

export async function create_group(name: string): Promise<InstanceGroupDefinition> {
	return await apiClient.local.call(instanceGroups.CreateInstanceGroup, { body: { name } })
}

export async function rename_group(id: string, newName: string): Promise<InstanceGroupDefinition> {
	return await apiClient.local.call(instanceGroups.RenameInstanceGroup, {
		params: { id },
		body: { name: newName },
	})
}

export async function delete_group(id: string): Promise<void> {
	return await apiClient.local.call(instanceGroups.DeleteInstanceGroup, { params: { id } })
}

export async function set_group_order(groupIds: string[]): Promise<void> {
	return await apiClient.local.call(instanceGroups.SetInstanceGroupOrder, { body: groupIds })
}

export async function set_group_memberships(
	updates: InstanceGroupMembershipUpdate[],
): Promise<void> {
	return await apiClient.local.call(instanceGroups.SetInstanceGroupMemberships, {
		body: updates,
	})
}
