<template>
	<PopoutMenu
		ref="dropdown"
		v-bind="$attrs"
		:disabled="disabled"
		:dropdown-id="dropdownId"
		:tooltip="tooltip"
		:placement="placement"
	>
		<slot />
		<template #menu>
			<slot name="menu-header" />
			<template v-for="(option, index) in options.filter((option) => option.shown !== false)">
				<div
					v-if="isDivider(option)"
					:key="`divider-${index}`"
					class="mx-3 my-2 h-px bg-surface-5"
				/>
				<Button
					v-else
					:key="`option-${option.id}`"
					v-tooltip="option.tooltip"
					:color="option.color ?? 'default'"
					:class="{ 'btn-hover-invert': option.hoverInvert }"
					:hover-filled="option.hoverFilled"
					:hover-filled-only="option.hoverFilledOnly"
					:transparent="!option.filled"
					:v-close-popper="!option.remainOnClick"
					:action="
						option.action
							? (event: MouseEvent) => {
									option.action?.(event)
									if (!option.remainOnClick) close()
								}
							: undefined
					"
					:link="option.link"
					:download="option.download"
					:external="option.external ?? false"
					:disabled="option.disabled"
					@click="() => option.link && !option.remainOnClick && close()"
				>
					<template v-if="!$slots[option.id]">
						<component :is="option.icon" v-if="option.icon" class="size-5" />
						{{ option.id }}
					</template>
					<slot :name="option.id" />
				</Button>
			</template>
		</template>
	</PopoutMenu>
</template>

<script setup lang="ts">
import { Button, PopoutMenu } from '@modrinth/ui'
import { type Component, type Ref, ref } from 'vue'

interface BaseOption {
	shown?: boolean
}

interface Divider extends BaseOption {
	divider?: boolean
}

interface Item extends BaseOption {
	id: string
	icon?: Component
	action?: (event?: MouseEvent) => void
	link?: string
	download?: string
	external?: boolean
	color?: 'primary' | 'danger' | 'secondary' | 'highlight' | 'red' | 'orange' | 'green' | 'blue' | 'purple'
	hoverFilled?: boolean
	hoverFilledOnly?: boolean
	filled?: boolean
	hoverInvert?: boolean
	remainOnClick?: boolean
	disabled?: boolean
	tooltip?: string
}

type Option = Divider | Item

withDefaults(
	defineProps<{
		options: Option[]
		disabled?: boolean
		dropdownId?: string
		tooltip?: string
		placement?: string
	}>(),
	{
		options: () => [],
		disabled: false,
		dropdownId: undefined,
		tooltip: undefined,
		placement: 'bottom-end',
	},
)

defineOptions({
	inheritAttrs: false,
})

const dropdown: Ref<InstanceType<typeof PopoutMenu> | null> = ref(null)

function close() {
	dropdown.value?.hide()
}

function isDivider(option: BaseOption): option is Divider {
	return 'divider' in option
}
</script>

<style scoped lang="scss">
.btn {
	width: 100%;
	justify-content: flex-start;
	white-space: nowrap;
	box-shadow: none;
	--text-color: var(--color-base);
	--background-color: transparent;

	&:not(:last-child) {
		margin-bottom: var(--gap-xs);
	}
}
</style>
