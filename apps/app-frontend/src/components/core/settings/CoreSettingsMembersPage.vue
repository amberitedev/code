<script setup lang="ts">
import { Combobox, SettingsLabel, Toggle, UnsavedChangesPopup } from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, ref } from 'vue'

const savedSettings = useStorage('amberite-core-member-settings', {
	defaultRole: 'member',
	allowMemberInvites: false,
	requireInviteApproval: true,
	allowFriendRequests: true,
})

const defaultRole = ref(savedSettings.value.defaultRole)
const allowMemberInvites = ref(savedSettings.value.allowMemberInvites)
const requireInviteApproval = ref(savedSettings.value.requireInviteApproval)
const allowFriendRequests = ref(savedSettings.value.allowFriendRequests)
const isUpdating = ref(false)

const roleOptions = [
	{ value: 'member', label: 'Member' },
	{ value: 'admin', label: 'Admin' },
]

const defaultRoleLabel = computed(
	() => roleOptions.find((option) => option.value === defaultRole.value)?.label ?? 'Member',
)
const modifiedSettings = computed(() => ({
	defaultRole: defaultRole.value,
	allowMemberInvites: allowMemberInvites.value,
	requireInviteApproval: requireInviteApproval.value,
	allowFriendRequests: allowFriendRequests.value,
}))

function saveMembers() {
	isUpdating.value = true
	savedSettings.value = { ...modifiedSettings.value }
	isUpdating.value = false
}

function resetMembers() {
	defaultRole.value = savedSettings.value.defaultRole
	allowMemberInvites.value = savedSettings.value.allowMemberInvites
	requireInviteApproval.value = savedSettings.value.requireInviteApproval
	allowFriendRequests.value = savedSettings.value.allowFriendRequests
}
</script>

<template>
	<div class="relative h-full w-full">
		<div class="flex max-w-[620px] flex-col gap-6 pb-10">
			<section class="flex flex-col gap-2.5">
				<SettingsLabel
					title="Members"
					description="Defaults for invites and new members joining this Core."
				/>
				<div class="flex flex-col gap-2.5">
					<label for="default-member-role" class="text-lg font-semibold text-contrast">
						Default role
					</label>
					<Combobox
						id="default-member-role"
						v-model="defaultRole"
						:options="roleOptions"
						:display-value="defaultRoleLabel"
						trigger-class="max-w-[240px]"
					/>
					<span>Role assigned to users who join through a standard invite.</span>
				</div>
			</section>

			<section class="flex flex-col gap-4">
				<div class="flex items-center justify-between gap-4">
					<label class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Members can invite</span>
						<span class="text-sm text-secondary">
							Allow regular members to create invites for friends.
						</span>
					</label>
					<Toggle v-model="allowMemberInvites" />
				</div>
				<div class="flex items-center justify-between gap-4">
					<label class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Require invite approval</span>
						<span class="text-sm text-secondary">
							New invite joins must be approved by an admin before access is granted.
						</span>
					</label>
					<Toggle v-model="requireInviteApproval" />
				</div>
				<div class="flex items-center justify-between gap-4">
					<label class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Send friend requests</span>
						<span class="text-sm text-secondary">
							Offer to add invited users as friends when access is granted.
						</span>
					</label>
					<Toggle v-model="allowFriendRequests" />
				</div>
			</section>
		</div>
		<UnsavedChangesPopup
			:original="savedSettings"
			:modified="modifiedSettings"
			:saving="isUpdating"
			@save="saveMembers"
			@reset="resetMembers"
		/>
	</div>
</template>
