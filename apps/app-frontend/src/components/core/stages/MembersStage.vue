<script setup lang="ts">
import { UserPlusIcon } from '@modrinth/assets'
import type { ComboboxOption } from '@modrinth/ui'
import { Avatar, ButtonStyled, Checkbox, Combobox } from '@modrinth/ui'
import { computed } from 'vue'

import { useSocial } from '@/composables/useSocial'

import CoreAccessTable from '../CoreAccessTable.vue'
import { injectCoreOnboardingContext } from '../core-onboarding-context'

const ctx = injectCoreOnboardingContext()
const social = useSocial()
const inviteSuggestionOptions = computed<ComboboxOption<string>[]>(() =>
	ctx.inviteSuggestions.value.map((user) => ({
		value: user.username,
		label: user.username,
		searchTerms: [user.username, user.id, user.email].filter(Boolean) as string[],
	})),
)
const inviteLookupMessage = computed(() => {
	const query = ctx.inviteSearch.value.trim()
	if (query.length < 2) return 'Type at least 2 characters to search.'
	if (ctx.inviteLookupStatus.value === 'loading') return 'Searching...'
	return 'No matching Amberite users.'
})
const friendRequestUnavailableIds = computed(
	() =>
		new Set([
			...(social.friends.value?.friends ?? [])
				.map((friend) => friend.user?.userId)
				.filter((id): id is string => !!id),
			...(social.friends.value?.incoming ?? []).map((request) => request.request.fromUserId),
			...(social.friends.value?.outgoing ?? []).map((request) => request.request.toUserId),
		]),
)
const canCreateInvite = computed(() => {
	const value = ctx.inviteSearch.value.trim().toLowerCase()
	if (!value) return false
	const user = ctx.inviteSuggestions.value.find(
		(suggestion) =>
			suggestion.username.toLowerCase() === value ||
			suggestion.id.toLowerCase() === value ||
			suggestion.email?.toLowerCase() === value,
	)
	return (
		!!user &&
		!ctx.members.value.some((member) => !member.inviteCandidate && member.user.id === user.id)
	)
})
const showFriendRequestToggle = computed(() => {
	const userId = ctx.selectedInviteSuggestion.value?.id
	return !!userId && !friendRequestUnavailableIds.value.has(userId)
})

function selectInviteOption(option: ComboboxOption<string>) {
	ctx.selectInviteSuggestion(
		ctx.inviteSuggestions.value.find((user) => user.username === option.value) ?? {
			id: option.value,
			username: option.label,
		},
	)
}
</script>

<template>
	<div class="flex min-h-[26rem] flex-col gap-4">
		<p class="m-0 text-secondary">
			Invite users by their unique Amberite username or their Minecraft username.
		</p>
		<div class="flex flex-col gap-3">
			<div class="flex flex-col gap-2 md:flex-row md:items-center">
				<div class="min-w-0 flex-1">
					<Combobox
						:model-value="ctx.selectedInviteSuggestion.value?.username"
						:options="inviteSuggestionOptions"
						:search-placeholder="'Search Amberite or Minecraft username'"
						:placeholder="'Search Amberite or Minecraft username'"
						:no-options-message="inviteLookupMessage"
						:min-search-length-to-open="2"
						:disable-search-filter="true"
						searchable
						show-search-icon
						:show-chevron="false"
						search-type="search"
						search-name="amberite-core-member-search"
						search-autocomplete="off"
						search-autocorrect="off"
						search-autocapitalize="none"
						:search-spellcheck="false"
						trigger-class="!h-10 !min-h-10"
						@search-input="(value) => (ctx.inviteSearch.value = value)"
						@select="selectInviteOption"
					>
						<template #option="{ item, isSelected }">
							<div class="flex min-w-0 items-center gap-2">
								<Avatar
									:src="
										ctx.inviteSuggestions.value.find((user) => user.username === item.value)
											?.avatarUrl
									"
									:alt="`${item.label}'s avatar`"
									:tint-by="item.label"
									size="1.5rem"
									circle
									no-shadow
								/>
								<span
									class="min-w-0 truncate font-semibold"
									:class="isSelected ? 'text-contrast' : 'text-primary'"
								>
									{{ item.label }}
								</span>
							</div>
						</template>
					</Combobox>
				</div>
				<ButtonStyled color="brand">
					<button
						class="!h-10 w-full md:w-fit"
						:disabled="!canCreateInvite"
						@click="ctx.createInvite"
					>
						<UserPlusIcon />
						Invite
					</button>
				</ButtonStyled>
			</div>
			<div class="flex items-center">
				<Checkbox
					v-if="showFriendRequestToggle"
					v-model="ctx.inviteAsFriend.value"
					label="Also send a friend request"
					label-class="text-base text-contrast"
				/>
			</div>
		</div>
		<CoreAccessTable
			:members="ctx.members.value"
			:roles="ctx.roles"
			:can-manage-users="ctx.canManage.value"
			status-column-label="Status"
			show-status-labels
			@update-role="ctx.updateRole"
			@invite-member="ctx.quickInvite"
			@cancel-invite="ctx.removeMember"
			@remove-member="ctx.removeMember"
		/>
	</div>
</template>
