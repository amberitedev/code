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
		friendRequestUnavailableIds?: string[]
		targetLabel?: string
		targetPlaceholder?: string
		targetHelp?: string
		permissionsHelp?: string
		permissionsHelpHref?: string
		permissionsHelpTarget?: string
	}>(),
	{
		members: () => [],
		suggestions: () => [],
		friendIds: () => [],
		friendRequestUnavailableIds: () => [],
		canGrant: true,
	},
)

const emit = defineEmits<{
	grant: [payload: GrantServerAccessPayload]
	permissionsHelpClick: [event: MouseEvent]
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
		:friend-request-unavailable-ids="props.friendRequestUnavailableIds"
		:search-users="props.searchUsers"
		:can-grant="props.canGrant"
		:target-label="props.targetLabel"
		:target-placeholder="props.targetPlaceholder"
		:target-help="props.targetHelp"
		:permissions-help="props.permissionsHelp"
		:permissions-help-href="props.permissionsHelpHref"
		:permissions-help-target="props.permissionsHelpTarget"
		@grant="emit('grant', $event)"
		@permissions-help-click="emit('permissionsHelpClick', $event)"
	/>
</template>
