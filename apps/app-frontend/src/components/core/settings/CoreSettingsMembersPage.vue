<script setup lang="ts">
import type { CoreRole } from '@amberite/amberite-api'
import { NavTabs, Table, Toggle, UnsavedChangesPopup } from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, onMounted, ref, watch } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { useConnectedCore } from '@/core/connected-core'

const permissions = [
	{ id: 'invite-members', name: 'Invite members', group: 'Members' },
	{ id: 'remove-members', name: 'Remove members', group: 'Members' },
	{ id: 'ban-members', name: 'Ban members', group: 'Members' },
	{ id: 'approve-invites', name: 'Approve invites', group: 'Members' },
	{ id: 'edit-member-roles', name: 'Edit member access', group: 'Members' },
	{ id: 'manage-instances', name: 'Manage instances', group: 'Instances' },
	{ id: 'start-stop-instances', name: 'Start and stop instances', group: 'Instances' },
	{ id: 'restart-instances', name: 'Restart instances', group: 'Instances' },
	{ id: 'manage-mods', name: 'Manage mods', group: 'Content' },
	{ id: 'manage-worlds', name: 'Manage worlds', group: 'Content' },
	{ id: 'manage-files', name: 'Manage files', group: 'Content' },
	{ id: 'manage-backups', name: 'Manage backups', group: 'Content' },
	{ id: 'manage-network', name: 'Manage networking', group: 'Core' },
	{ id: 'read-console', name: 'Read console', group: 'Core' },
	{ id: 'write-console', name: 'Write console', group: 'Core' },
	{ id: 'edit-settings', name: 'Edit Core settings', group: 'Core' },
	{ id: 'view-activity', name: 'View activity', group: 'Core' },
]

const defaultRoles: CoreRole[] = [
	{ id: 'role-member', name: 'Member', description: 'Uses shared Core access.', icon: 'user', grants_json: JSON.stringify(['start-stop-instances', 'restart-instances', 'read-console', 'view-activity']), created_at: '1970-01-01T00:00:00.000Z', updated_at: '1970-01-01T00:00:00.000Z' },
	{ id: 'role-admin', name: 'Admin', description: 'Manages members and Core settings.', icon: 'shield', grants_json: JSON.stringify(permissions.map((permission) => permission.id)), created_at: '1970-01-01T00:00:00.000Z', updated_at: '1970-01-01T00:00:00.000Z' },
]

const core = useCoreClient()
const connectedCore = useConnectedCore()
const localRoles = useStorage<CoreRole[]>('amberite:core-access-defaults', cloneRoles(defaultRoles))
const roles = ref<CoreRole[]>([])
const selectedId = ref('role-member')
const draftGrants = ref<string[]>([])
const isLoading = ref(false)
const isSaving = ref(false)
const usingLocalRoles = ref(false)

const selectedRole = computed(() => roles.value.find((role) => role.id === selectedId.value))
const roleTabs = computed(() => roles.value.map((role) => ({ label: role.name, href: role.id })))
const groupedPermissions = computed(() => {
	const groups = new Map<string, typeof permissions>()
	for (const permission of permissions) groups.set(permission.group, [...(groups.get(permission.group) ?? []), permission])
	return [...groups]
})
const permissionColumns = [
	{ key: 'name', label: 'Permission' },
	{ key: 'enabled', label: 'Allowed', align: 'right' as const, width: '7rem' },
]
const originalDraft = computed(() => ({ grants: parseGrants(selectedRole.value?.grants_json ?? '[]') }))
const modifiedDraft = computed(() => ({ grants: draftGrants.value }))

watch(selectedRole, (role) => {
	draftGrants.value = parseGrants(role?.grants_json ?? '[]')
}, { immediate: true })

async function load() {
	isLoading.value = true
	try {
		if (!connectedCore.value) {
			loadLocalRoles()
			return
		}
		const configuration = await core.getCoreRoles()
		roles.value = fixedRoles(configuration.roles)
		usingLocalRoles.value = false
	} catch {
		loadLocalRoles()
	} finally {
		isLoading.value = false
	}
}

async function saveSelected() {
	if (!selectedRole.value) return
	isSaving.value = true
	try {
		const input = {
			id: selectedRole.value.id,
			name: selectedRole.value.name,
			description: selectedRole.value.description,
			icon: selectedRole.value.icon,
			grants: draftGrants.value,
		}
		const role = usingLocalRoles.value ? saveLocalRole(input) : await core.saveCoreRole(input)
		roles.value = roles.value.map((item) => item.id === role.id ? role : item)
		if (usingLocalRoles.value) localRoles.value = cloneRoles(roles.value)
	} finally {
		isSaving.value = false
	}
}

function resetSelected() {
	draftGrants.value = parseGrants(selectedRole.value?.grants_json ?? '[]')
}

function loadLocalRoles() {
	roles.value = fixedRoles(localRoles.value)
	usingLocalRoles.value = true
}

function toggleGrant(id: string, enabled: boolean) {
	draftGrants.value = enabled ? [...new Set([...draftGrants.value, id])] : draftGrants.value.filter((grant) => grant !== id)
}

function selectRole(tab: { href: string }) {
	selectedId.value = tab.href
}

function fixedRoles(source: CoreRole[]) {
	return defaultRoles.map((defaultRole) => source.find((role) => role.id === defaultRole.id) ?? defaultRole)
}

function saveLocalRole(input: { id?: string; name: string; description: string; icon: string; grants: string[] }): CoreRole {
	const existing = roles.value.find((role) => role.id === input.id)
	return {
		id: input.id ?? `local-role-${crypto.randomUUID()}`,
		name: input.name,
		description: input.description,
		icon: input.icon,
		grants_json: JSON.stringify(input.grants),
		created_at: existing?.created_at ?? new Date().toISOString(),
		updated_at: new Date().toISOString(),
	}
}

function parseGrants(value: string): string[] {
	try { return JSON.parse(value) as string[] } catch { return [] }
}

function cloneRoles(source: CoreRole[]) {
	return source.map((role) => ({ ...role, grants_json: JSON.stringify(parseGrants(role.grants_json)) }))
}

watch(connectedCore, () => void load())
onMounted(load)
</script>

<template>
	<div class="relative flex h-full w-full flex-col gap-5">
		<div class="flex min-w-0 flex-wrap items-center gap-3">
			<NavTabs
				mode="local"
				class="!bg-surface-3"
				:links="roleTabs"
				:active-index="Math.max(0, roleTabs.findIndex((role) => role.href === selectedId))"
				@tab-click="(_index, tab) => selectRole(tab)"
			/>
		</div>

		<section v-if="selectedRole" class="flex flex-col gap-5 pb-10">
			<div v-for="[group, items] in groupedPermissions" :key="group">
				<Table :columns="permissionColumns" :data="items" row-key="id">
					<template #header>
						<span class="text-lg font-extrabold text-contrast">{{ group }}</span>
					</template>
					<template #cell-name="{ value }">
						<span class="font-semibold text-contrast">{{ value }}</span>
					</template>
					<template #cell-enabled="{ row }">
						<Toggle :model-value="draftGrants.includes(row.id)" @update:model-value="toggleGrant(row.id, $event)" />
					</template>
				</Table>
			</div>
		</section>
		<div v-else class="flex flex-1 items-center justify-center text-secondary">{{ isLoading ? 'Loading access defaults…' : 'Access defaults are unavailable.' }}</div>
		<UnsavedChangesPopup :original="originalDraft" :modified="modifiedDraft" :saving="isSaving" @save="saveSelected" @reset="resetSelected" />
	</div>
</template>
