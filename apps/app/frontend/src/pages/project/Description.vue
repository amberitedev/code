<template>
	<Card>
		<ProjectPageDescription :description="project.body" />
	</Card>
</template>

<script setup>
import { Card, ProjectPageDescription } from '@amberite/ui'

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
}
</script>
