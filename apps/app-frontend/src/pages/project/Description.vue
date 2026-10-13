<template>
	<Card>
		<ProjectPageDescription :description="project.body" />
	</Card>
</template>

<script setup>
import { Card, ProjectPageDescription } from '@modrinth/ui'

defineProps({
	project: {
		type: Object,
		default: () => {},
	},
})
</script>

<script>
export default {
	name: 'Description',
	// The parent route view passes every tab's props; the rest must not land on the card as attributes.
	inheritAttrs: false,
}
</script>
