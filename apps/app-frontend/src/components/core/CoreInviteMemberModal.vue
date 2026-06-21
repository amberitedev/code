<script setup lang="ts">
import type { CoreRole } from '@amberite/amberite-api'
import { RightArrowIcon, UserPlusIcon, XIcon } from '@modrinth/assets'
import { ButtonStyled, Combobox, NewModal } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useSocialClient } from '@/composables/useSocialClient'

const props = defineProps<{ roles: CoreRole[] }>()
const emit = defineEmits<{ invite: [user: { id: string; username: string }, roleId: string]; 'open-role': [roleId: string] }>()
const social = useSocialClient()
const modal = ref<InstanceType<typeof NewModal>>()
const search = ref('')
const users = ref<Array<{ id: string; username: string }>>([])
const selectedUser = ref<{ id: string; username: string }>()
const selectedRoleId = ref('')
const loading = ref(false)
const options = computed(() => users.value.map((user) => ({ value: user.id, label: user.username })))
const selectedRole = computed(() => props.roles.find((role) => role.id === selectedRoleId.value))

async function searchUsers(value: string) {
	search.value = value
	if (!value.trim()) return
	loading.value = true
	try {
		users.value = (await social.searchUsers(value).catch(() => [])).map((user) => ({ id: user.userId, username: user.username ?? user.displayName ?? user.userId }))
	} finally { loading.value = false }
}

function selectUser(option: { value: string; label: string }) { selectedUser.value = { id: option.value, username: option.label } }
function show() { selectedUser.value = undefined; selectedRoleId.value = props.roles[0]?.id ?? ''; users.value = []; modal.value?.show() }
function submit() { if (!selectedUser.value || !selectedRole.value) return; emit('invite', selectedUser.value, selectedRole.value.id); modal.value?.hide() }
defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" header="Invite friends" max-width="38rem">
		<div class="flex flex-col gap-5">
			<div class="flex flex-col gap-2"><label class="font-semibold text-contrast">Amberite username</label><Combobox :model-value="undefined" :options="options" searchable search-placeholder="Search Amberite users" :no-options-message="loading ? 'Searching…' : 'No matching users found.'" @search-input="searchUsers" @select="selectUser" /><span class="text-sm text-secondary">{{ selectedUser ? `Inviting ${selectedUser.username}` : 'Search for a user by their Amberite username.' }}</span></div>
			<div class="flex flex-col gap-2"><span class="font-semibold text-contrast">Select role</span><div class="grid gap-2 sm:grid-cols-2"><div v-for="role in roles" :key="role.id" class="relative rounded-xl border border-solid p-3" :class="selectedRoleId === role.id ? 'border-brand bg-brand-highlight' : 'border-surface-5 bg-surface-4'"><button class="absolute inset-0 rounded-xl" :aria-label="`Select ${role.name}`" @click="selectedRoleId = role.id" /><button class="absolute right-2 top-2 z-10 rounded p-1 text-secondary hover:text-contrast" :aria-label="`Open ${role.name} settings`" @click.stop="emit('open-role', role.id)"><RightArrowIcon class="size-4" /></button><div class="pointer-events-none flex flex-col gap-1 pr-5"><span class="font-semibold text-contrast">{{ role.name }}</span><span class="text-sm text-primary">{{ role.description }}</span></div></div></div></div>
		</div>
		<template #actions><div class="flex justify-end gap-2"><ButtonStyled><button @click="modal?.hide()"><XIcon />Cancel</button></ButtonStyled><ButtonStyled color="brand"><button :disabled="!selectedUser || !selectedRole" @click="submit"><UserPlusIcon />Invite</button></ButtonStyled></div></template>
	</NewModal>
</template>
