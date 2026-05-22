<script setup>
import { BanIcon, CrownIcon, UserPlusIcon, UsersIcon } from '@modrinth/assets'
import { ButtonStyled, StyledInput } from '@modrinth/ui'
import { ref } from 'vue'

defineProps({
	groupMembers: { type: Array, required: true },
	localMembers: { type: Array, required: true },
})

const emit = defineEmits(['add-friend', 'add-local-member', 'remove-member'])
const friendSearch = ref('')
const memberId = ref('')
const memberName = ref('')

function addFriend() {
	const target = friendSearch.value.trim()
	if (!target) return
	emit('add-friend', target)
	friendSearch.value = ''
}

function addLocalMember() {
	const userId = memberId.value.trim()
	if (!userId) return
	emit('add-local-member', { userId, displayName: memberName.value.trim() || null })
	memberId.value = ''
	memberName.value = ''
}
</script>

<template>
	<section class="grid gap-4 md:grid-cols-2">
		<div class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5">
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<UserPlusIcon /> Add friend
			</h2>
			<StyledInput
				v-model="friendSearch"
				placeholder="Username or AMB code"
				wrapper-class="w-full"
			/>
			<ButtonStyled
				><button class="mt-3" @click="addFriend">Send friend request</button></ButtonStyled
			>
		</div>

		<div class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5">
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<UsersIcon /> Add group member
			</h2>
			<StyledInput v-model="memberId" placeholder="Amberite user ID" wrapper-class="w-full" />
			<StyledInput
				v-model="memberName"
				class="mt-2"
				placeholder="Display name"
				wrapper-class="w-full"
			/>
			<ButtonStyled
				><button class="mt-3" @click="addLocalMember">
					Create permission record
				</button></ButtonStyled
			>
		</div>

		<div class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5">
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<CrownIcon /> Local permissions
			</h2>
			<div v-for="member in localMembers" :key="member.user_id" class="member-row">
				<span>{{ member.display_name ?? member.user_id }}</span>
				<span>{{ member.role }} / {{ member.permission_preset }}</span>
				<button @click="$emit('remove-member', member.user_id)"><BanIcon /> Remove</button>
			</div>
		</div>

		<div class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5">
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<UsersIcon /> Convex group
			</h2>
			<div v-for="member in groupMembers" :key="member.userId" class="member-row">
				<span>{{ member.user?.displayName ?? member.user?.username ?? member.userId }}</span>
				<span>{{ member.role }} / {{ member.permissionPreset ?? member.role }}</span>
			</div>
			<p v-if="groupMembers.length === 0" class="m-0 text-secondary">No group members loaded.</p>
		</div>
	</section>
</template>

<style scoped>
.member-row {
	@apply flex flex-wrap items-center justify-between gap-3 border-0 border-b border-solid border-button-border py-3;
}
.member-row button {
	@apply inline-flex items-center gap-2 rounded-full border border-solid border-button-border bg-bg px-3 py-1 text-primary;
}
</style>
