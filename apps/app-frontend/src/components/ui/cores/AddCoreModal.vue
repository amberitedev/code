<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { RightArrowIcon } from '@modrinth/assets'
import { Button, injectModrinthClient, NewModal } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { coresQueryKey, registerAddCoreModal } from '@/composables/use-cores'

// The one "Add Core" modal, mounted once in App.vue. Open it with `openAddCoreModal` or
// `ensureCore` from `@/composables/use-cores`. The user types the code their Core shows, sees
// which machine is asking, and approves it; the Core then signs in on its own.

const client = injectModrinthClient()
const queryClient = useQueryClient()

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const code = ref('')
// Which Core the code belongs to, once looked up. Approval is the second step.
const pending = ref<Archon.Cores.v1.PendingCore | null>(null)
const error = ref('')
const working = ref(false)
let settle: ((added: boolean) => void) | null = null

const inputs = ref<HTMLInputElement[]>([])
const codeError = ref('')
const isInvalid = ref(false)
const shouldAnimateError = ref(false)
let errorOutlineTimeout: ReturnType<typeof setTimeout> | undefined

const characters = computed(() => {
	const raw = code.value.replace(/[^A-Z0-9]/gi, '').toUpperCase()
	return Array.from({ length: 8 }, (_, index) => raw[index] ?? '')
})
const complete = computed(() => characters.value.every(Boolean))

watch(code, () => {
	clearErrorState()
})

watch(error, (message) => {
	if (!message) return
	triggerErrorFeedback()
})

onMounted(() => registerAddCoreModal(show))
onBeforeUnmount(() => registerAddCoreModal(null))

/** Open the modal. With `prefilled` (from an `amberite://approve` link) it looks the code up at once. */
function show(prefilled = ''): Promise<boolean> {
	settle?.(false)
	code.value = prefilled
		.replace(/[^A-Z0-9]/gi, '')
		.toUpperCase()
		.slice(0, 8)
	pending.value = null
	clearErrorState()
	modal.value?.show()
	if (complete.value) void lookup()
	else void focusInput(0)
	return new Promise((resolve) => {
		settle = resolve
	})
}

function handleHide() {
	settle?.(false)
	settle = null
}

async function lookup() {
	if (working.value || !complete.value) return
	working.value = true
	error.value = ''
	try {
		pending.value = await client.archon.cores_v1.lookup(code.value)
	} catch (reason) {
		error.value =
			reason instanceof Error ? reason.message : 'Could not add the Core. Please try again.'
	} finally {
		working.value = false
	}
}

async function approve() {
	if (working.value || !pending.value) return
	working.value = true
	error.value = ''
	try {
		await client.archon.cores_v1.approve(code.value)
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: coresQueryKey }),
			queryClient.invalidateQueries({ queryKey: ['servers'] }),
		])
		settle?.(true)
		settle = null
		modal.value?.hide()
	} catch (reason) {
		error.value =
			reason instanceof Error ? reason.message : 'Could not add the Core. Please try again.'
	} finally {
		working.value = false
	}
}

function back() {
	pending.value = null
	clearErrorState()
	void focusInput(0)
}

function setInputRef(element: unknown, index: number) {
	if (element instanceof HTMLInputElement) inputs.value[index] = element
}

function updateCode(value: string, startIndex = 0) {
	const raw = value.replace(/[^A-Z0-9]/gi, '').toUpperCase()
	if (raw.length > 8 - startIndex) {
		showError('A Core code must contain exactly 8 letters or numbers.')
		return
	}
	const next = [...characters.value]
	next.splice(startIndex, raw.length, ...raw)
	code.value = next.join('').slice(0, 8)
	const nextIndex = startIndex + raw.length
	if (nextIndex < 8) void focusInput(nextIndex)
	else (document.activeElement as HTMLElement | null)?.blur()
}

function handleInput(event: Event, index: number) {
	const target = event.target as HTMLInputElement
	const typed = target.value
	if (typed && !/[a-z0-9]/i.test(typed)) {
		target.value = characters.value[index]
		return
	}
	if (!typed) {
		const next = [...characters.value]
		next[index] = ''
		code.value = next.join('')
		return
	}
	updateCode(typed, index)
}

function handlePaste(event: ClipboardEvent, index: number) {
	event.preventDefault()
	updateCode(event.clipboardData?.getData('text') ?? '', index)
}

function handleKeydown(event: KeyboardEvent, index: number) {
	if (event.key === 'Enter') {
		void lookup()
		return
	}
	if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !/[a-z0-9]/i.test(event.key)) {
		event.preventDefault()
		return
	}
	if (event.key === 'Backspace' && !characters.value[index] && index > 0) {
		event.preventDefault()
		const next = [...characters.value]
		next[index - 1] = ''
		code.value = next.join('')
		void focusInput(index - 1)
	} else if (event.key === 'ArrowLeft' && index > 0) {
		event.preventDefault()
		void focusInput(index - 1)
	} else if (event.key === 'ArrowRight' && index < 7) {
		event.preventDefault()
		void focusInput(index + 1)
	}
}

function handleBeforeInput(event: InputEvent) {
	if (event.data && !/^[a-z0-9]+$/i.test(event.data)) event.preventDefault()
}

function selectInput(event: FocusEvent) {
	clearErrorState()
	;(event.target as HTMLInputElement).select()
}

function focusFirstInput(event: PointerEvent, index: number) {
	if (index === 0 || characters.value[index]) return
	const firstEmptyIndex = characters.value.findIndex((character) => !character)
	if (firstEmptyIndex === -1 || firstEmptyIndex === index) return
	event.preventDefault()
	void focusInput(firstEmptyIndex)
}

function showError(message: string) {
	codeError.value = message
	triggerErrorFeedback()
}

function triggerErrorFeedback() {
	if (errorOutlineTimeout) clearTimeout(errorOutlineTimeout)
	isInvalid.value = true
	shouldAnimateError.value = false
	void nextTick(() => {
		shouldAnimateError.value = true
	})
	errorOutlineTimeout = setTimeout(() => {
		isInvalid.value = false
	}, 600)
}

function clearErrorState() {
	if (errorOutlineTimeout) clearTimeout(errorOutlineTimeout)
	codeError.value = ''
	error.value = ''
	isInvalid.value = false
	shouldAnimateError.value = false
}

function clearErrorAnimation(event: AnimationEvent) {
	if (event.target === event.currentTarget) shouldAnimateError.value = false
}

async function focusInput(index: number) {
	await nextTick()
	inputs.value[index]?.focus()
}
</script>

<template>
	<NewModal ref="modal" header="Add Core" max-width="800px" width="100%" :on-hide="handleHide">
		<div v-if="pending" class="flex min-h-[26rem] flex-col gap-4">
			<p class="m-0 text-center text-lg font-medium text-primary">
				Sign this Core in to your account?
			</p>
			<div class="flex flex-1 flex-col items-center justify-center gap-2">
				<span class="text-3xl font-bold text-contrast">{{ pending.name }}</span>
				<span class="font-mono text-secondary">{{ pending.address }}</span>
				<p class="m-0 mt-4 max-w-md text-center text-sm text-secondary">
					Only approve a code shown by a Core you set up yourself. The Core will be able to host
					servers under your account.
				</p>
				<p class="m-0 min-h-5 text-center text-sm text-red" :role="error ? 'alert' : undefined">
					{{ error }}
				</p>
			</div>
		</div>
		<div v-else class="flex min-h-[26rem] flex-col gap-4">
			<p class="m-0 text-center text-lg font-medium text-primary">
				Enter the code shown by the Core you want to add.
			</p>
			<div class="flex flex-1 flex-col items-center justify-center gap-4">
				<div
					class="flex items-center gap-3"
					:class="{ 'animate-shake': shouldAnimateError }"
					@animationend="clearErrorAnimation"
				>
					<template v-for="(_, index) in characters" :key="index">
						<span v-if="index === 4" class="text-5xl font-bold text-contrast" aria-hidden="true"
							>-</span
						>
						<input
							:ref="(element) => setInputRef(element, index)"
							:value="characters[index]"
							type="text"
							inputmode="text"
							autocomplete="one-time-code"
							autocapitalize="characters"
							spellcheck="false"
							maxlength="1"
							:aria-label="`Code character ${index + 1}`"
							:disabled="working"
							class="h-28 w-20 rounded-xl border border-solid bg-surface-4 text-center font-mono text-5xl font-bold text-contrast outline-none transition focus:border-brand focus:ring-4 focus:ring-brand-shadow"
							:class="
								isInvalid
									? ['!border-red !ring-4 !ring-red', { 'animate-code-error': shouldAnimateError }]
									: ['border-button-bg', working ? 'cursor-not-allowed opacity-70' : '']
							"
							@beforeinput="handleBeforeInput"
							@input="handleInput($event, index)"
							@paste="handlePaste($event, index)"
							@keydown="handleKeydown($event, index)"
							@pointerdown="focusFirstInput($event, index)"
							@focus="selectInput"
						/>
					</template>
				</div>
				<p
					class="m-0 min-h-5 text-center text-sm text-red"
					:role="codeError || error ? 'alert' : undefined"
				>
					{{ codeError || error }}
				</p>
			</div>
			<p class="m-0 text-center text-sm text-secondary">
				Don't have a Core yet?
				<a
					class="font-medium text-brand hover:underline"
					href="https://github.com/amberitedev/code#readme"
					target="_blank"
					rel="noopener noreferrer"
				>
					Learn how to set one up.
				</a>
			</p>
		</div>
		<template #actions>
			<div v-if="pending" class="flex justify-between">
				<Button :disabled="working" @click="back">Back</Button>
				<Button type="colored" color="brand" :disabled="working" @click="approve">
					{{ working ? 'Approving' : 'Approve' }}
					<RightArrowIcon aria-hidden="true" />
				</Button>
			</div>
			<div v-else class="flex justify-end">
				<Button type="colored" color="brand" :disabled="working || !complete" @click="lookup">
					{{ working ? 'Checking' : 'Next' }}
					<RightArrowIcon aria-hidden="true" />
				</Button>
			</div>
		</template>
	</NewModal>
</template>

<style scoped>
@keyframes shake {
	0%,
	100% {
		transform: translateX(0);
	}
	33% {
		transform: translateX(-0.25rem);
	}
	66% {
		transform: translateX(0.25rem);
	}
}

.animate-shake {
	animation: shake 300ms ease-in-out;
}

.animate-code-error {
	animation: flash-error 250ms ease-in-out;
}

@keyframes flash-error {
	0%,
	100% {
		opacity: 1;
	}
	50% {
		opacity: 0.45;
	}
}
</style>
