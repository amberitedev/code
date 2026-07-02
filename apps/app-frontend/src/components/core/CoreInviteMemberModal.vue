<script setup lang="ts">
import type {
	GrantServerAccessPayload,
	ServerAccessInviteSuggestion,
	ServerAccessMember,
} from '@modrinth/ui'
import { GrantAccessModal } from '@modrinth/ui'
import { ref } from 'vue'

const props = withDefaults(
	defineProps<{
		members?: ServerAccessMember[]
		suggestions?: ServerAccessInviteSuggestion[]
		friendIds?: string[]
		searchUsers?: (query: string) => Promise<ServerAccessInviteSuggestion[]>
		canGrant?: boolean
	}>(),
	{
		members: () => [],
		suggestions: () => [],
		friendIds: () => [],
		canGrant: true,
	},
)

const emit = defineEmits<{
	grant: [payload: GrantServerAccessPayload]
}>()

const modal = ref<InstanceType<typeof GrantAccessModal> | null>(null)

function show(event?: MouseEvent) {
	modal.value?.show(event)
}

function hide() {
	modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
	<GrantAccessModal
		ref="modal"
		:members="props.members"
		:suggestions="props.suggestions"
		:friend-ids="props.friendIds"
		:search-users="props.searchUsers"
		:can-grant="props.canGrant"
		@grant="emit('grant', $event)"
	/>
</template>
