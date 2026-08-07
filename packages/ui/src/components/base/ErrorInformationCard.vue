<template>
	<div class="error-information-card flex max-w-lg flex-col items-center rounded-3xl bg-bg-raised p-8 shadow-xl">
		<div class="flex flex-col items-center text-center">
			<div class="flex flex-col items-center gap-4">
				<div class="grid place-content-center rounded-full bg-bg-orange p-4">
					<component :is="icon" class="size-12 text-orange" />
				</div>
				<h1 class="m-0 mb-2 w-fit text-4xl font-bold">{{ title }}</h1>
			</div>
			<div v-if="!description">
				<slot name="description" />
			</div>
			<p v-else class="text-lg text-secondary">{{ description }}</p>
		</div>

		<div v-if="errorDetails" class="my-4 w-full rounded-lg border border-divider bg-bg-raised">
			<div class="divide-y divide-divider">
				<div
					v-for="detail in errorDetails.filter((detail) => detail.type !== 'hidden')"
					:key="detail.label"
					class="px-4 py-3"
				>
					<div v-if="detail.type === 'inline'" class="flex items-center justify-between">
						<span class="font-medium text-secondary">{{ detail.label }}</span>
						<div class="flex items-center gap-2">
							<code class="rounded-lg bg-code-bg px-2 py-1 text-sm text-code-text">
								{{ detail.value }}
							</code>
						</div>
					</div>

					<div v-else-if="detail.type === 'block'" class="flex flex-col gap-2">
						<div class="flex items-center justify-between">
							<span class="font-medium text-secondary">{{ detail.label }}</span>
						</div>
						<div class="w-full overflow-hidden rounded-lg bg-code-bg p-3">
							<code
								class="block w-full overflow-x-auto break-words text-sm text-code-text whitespace-pre-wrap"
							>
								{{ detail.value }}
							</code>
						</div>
					</div>
				</div>
			</div>
		</div>

		<div class="mt-4 flex !w-full flex-row gap-4">
			<Button
				v-if="action"
				:type="action.color === 'standard' ? 'base' : 'colored'"
				:color="action.color === 'standard' ? undefined : (action.color ?? 'brand')"
				size="xl"
				:disabled="action.disabled"
				class="flex-1"
				@click="action.onClick"
			>
				<component :is="action.icon" v-if="action.icon && !action.showAltIcon" class="size-4" />
				<component
					:is="action.altIcon"
					v-else-if="action.icon && action.showAltIcon"
					class="size-4"
				/>
				{{ action.label }}
			</Button>

			<Button v-if="errorDetails" size="xl" class="flex-1" @click="copyErrorInformation">
				<CopyIcon v-if="!infoCopied" class="size-4" />
				<CheckIcon v-else class="size-4" />
				Copy Information
			</Button>
		</div>
	</div>
</template>

<script setup lang="ts">
import { CheckIcon, CopyIcon } from '@modrinth/assets'
import type { Component } from 'vue'
import { ref } from 'vue'

import { Button } from '#ui/components/base/buttons'

const infoCopied = ref(false)

const props = defineProps<{
	title: string
	description?: string
	icon: Component
	errorDetails?: {
		label?: string
		value?: string
		type?: 'inline' | 'block' | 'hidden'
	}[]
	action?: {
		label: string
		onClick: () => void
		color?: 'brand' | 'standard' | 'red' | 'orange' | 'blue'
		disabled?: boolean
		icon?: Component
		altIcon?: Component
		showAltIcon?: boolean
	}
}>()

const copyErrorInformation = async () => {
	if (!props.errorDetails || props.errorDetails.length === 0) return

	const formattedErrorInfo = props.errorDetails
		.filter((detail) => detail.label && detail.value)
		.map((detail) => `${detail.label}: ${detail.value}`)
		.join('\n\n')

	await navigator.clipboard.writeText(formattedErrorInfo)
	infoCopied.value = true
	setTimeout(() => {
		infoCopied.value = false
	}, 2000)
}
</script>

<style scoped lang="scss">
@media (prefers-reduced-motion: no-preference) {
	.error-information-card {
		animation: error-information-card-enter 180ms cubic-bezier(0.2, 0.8, 0.2, 1) both;
		transform-origin: center;
	}

	@keyframes error-information-card-enter {
		from {
			opacity: 0;
			transform: translate3d(0, 0.75rem, 0) scale(0.98);
		}

		to {
			opacity: 1;
			transform: translate3d(0, 0, 0) scale(1);
		}
	}
}
</style>
