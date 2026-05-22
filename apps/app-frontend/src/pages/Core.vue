<script setup>
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import CoreHeroCard from '@/components/ui/core/CoreHeroCard.vue'
import CoreMembersTab from '@/components/ui/core/CoreMembersTab.vue'
import CoreOverviewTab from '@/components/ui/core/CoreOverviewTab.vue'
import CorePermissionsTab from '@/components/ui/core/CorePermissionsTab.vue'
import CoreSyncTab from '@/components/ui/core/CoreSyncTab.vue'
import CoreTabNav from '@/components/ui/core/CoreTabNav.vue'
import { ensureAmberiteSession } from '@/helpers/amberite-auth'
import {
	getCoreMetadata,
	listCoreMembers,
	listPermissionPresets,
	listSyncProfiles,
	registerSyncProfile,
	removeCoreMember,
	removeSyncProfile,
	updateCoreMetadata,
	upsertCoreMember,
} from '@/helpers/amberite-core'
import {
	createFriendGroupInvite,
	friendGroupId,
	listFriendGroupMembers,
	listMyFriendGroups,
	sendFriendRequest,
	updateFriendGroup,
	updateMemberRole,
} from '@/helpers/friend-groups'

defineOptions({
	name: 'CorePage',
})

const router = useRouter()
const selectedTab = ref('overview')
const tabs = ['overview', 'members', 'permissions', 'sync']
const loading = ref(true)
const error = ref(null)
const inviteCode = ref(null)

const core = ref(null)
const groups = ref([])
const activeGroup = computed(() => groups.value[0])
const members = ref([])
const localMembers = ref([])
const presets = ref([])
const syncProfiles = ref([])

onMounted(loadCorePage)

async function loadCorePage() {
	loading.value = true
	error.value = null
	await ensureAmberiteSession().catch(() => null)
	core.value = await getCoreMetadata().catch((err) => {
		error.value = err instanceof Error ? err.message : String(err)
		return null
	})
	groups.value = await listMyFriendGroups().catch(() => [])
	const groupId = activeGroup.value ? friendGroupId(activeGroup.value) : ''
	const [groupMembers, coreMembers, permissionPresets, profiles] = await Promise.all([
		groupId ? listFriendGroupMembers(groupId).catch(() => []) : [],
		listCoreMembers().catch(() => []),
		listPermissionPresets().catch(() => []),
		listSyncProfiles().catch(() => []),
	])
	members.value = groupMembers
	localMembers.value = Array.isArray(coreMembers) ? coreMembers : (coreMembers.members ?? [])
	presets.value = permissionPresets
	syncProfiles.value = Array.isArray(profiles) ? profiles : (profiles.profiles ?? [])
	loading.value = false
}

async function saveCore() {
	if (!core.value) return
	core.value = await updateCoreMetadata({
		name: core.value.name,
		description: core.value.description,
		banner: core.value.banner,
		subdomain: core.value.subdomain,
	})
	const groupId = activeGroup.value ? friendGroupId(activeGroup.value) : ''
	if (groupId)
		await updateFriendGroup({
			friendGroupId: groupId,
			name: core.value.name,
			description: core.value.description,
			banner: core.value.banner,
			subdomain: core.value.subdomain,
		})
}

async function createInvite() {
	if (!activeGroup.value) return
	const groupId = friendGroupId(activeGroup.value)
	if (!groupId) return
	const invite = await createFriendGroupInvite({ friendGroupId: groupId })
	inviteCode.value = invite.code ?? null
}

async function addFriend(target) {
	await sendFriendRequest(target.includes('AMB-') ? { friendCode: target } : { username: target })
}

async function addLocalMember({ userId, displayName }) {
	const member = await upsertCoreMember({
		user_id: userId,
		display_name: displayName,
		role: 'member',
		permission_preset: 'member',
	})
	localMembers.value = [...localMembers.value.filter((x) => x.user_id !== member.user_id), member]
}

async function setMemberPreset(userId, presetId) {
	const preset = presets.value.find((item) => item.id === presetId)
	const member = await upsertCoreMember({
		user_id: userId,
		role: preset?.role ?? 'member',
		permission_preset: presetId,
	})
	localMembers.value = localMembers.value.map((item) => (item.user_id === userId ? member : item))
	const groupId = activeGroup.value ? friendGroupId(activeGroup.value) : ''
	if (groupId)
		await updateMemberRole({
			friendGroupId: groupId,
			userId,
			role: member.role,
			permissionPreset: presetId,
		})
}

async function removeMember(userId) {
	await removeCoreMember(userId)
	localMembers.value = localMembers.value.filter((member) => member.user_id !== userId)
}

async function addSyncProfile(body) {
	const profile = await registerSyncProfile({ ...body, sync_enabled: true })
	syncProfiles.value = [profile, ...syncProfiles.value.filter((item) => item.id !== profile.id)]
}

async function deleteSyncProfile(profileId) {
	await removeSyncProfile(profileId)
	syncProfiles.value = syncProfiles.value.filter((profile) => profile.id !== profileId)
}
</script>

<template>
	<div
		class="min-h-full bg-[radial-gradient(circle_at_top_left,rgba(32,220,160,0.18),transparent_32rem)] p-6"
	>
		<div class="mx-auto flex max-w-7xl flex-col gap-5">
			<p v-if="loading" class="m-0 rounded-2xl bg-bg-raised p-3 font-semibold text-secondary">
				Loading Core details...
			</p>
			<p v-if="error" class="m-0 rounded-2xl bg-red-highlight p-3 font-semibold text-red">
				{{ error }}
			</p>
			<CoreHeroCard :core-name="core?.name" @setup="router.push('/core/setup')" />

			<CoreTabNav v-model="selectedTab" :tabs="tabs" />

			<CoreOverviewTab
				v-if="selectedTab === 'overview'"
				v-model:core="core"
				:invite-code="inviteCode"
				:local-members-count="localMembers.length"
				:members-count="members.length"
				@create-invite="createInvite"
				@save-core="saveCore"
			/>
			<CoreMembersTab
				v-else-if="selectedTab === 'members'"
				:group-members="members"
				:local-members="localMembers"
				@add-friend="addFriend"
				@add-local-member="addLocalMember"
				@remove-member="removeMember"
			/>
			<CorePermissionsTab
				v-else-if="selectedTab === 'permissions'"
				:local-members="localMembers"
				:presets="presets"
				@set-member-preset="setMemberPreset"
			/>
			<CoreSyncTab
				v-else
				:sync-profiles="syncProfiles"
				@register-profile="addSyncProfile"
				@remove-profile="deleteSyncProfile"
			/>
		</div>
	</div>
</template>
