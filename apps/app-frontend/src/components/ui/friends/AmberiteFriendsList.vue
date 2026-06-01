<script setup lang="ts">
import { LogInIcon, MailIcon, UserPlusIcon, XIcon } from '@modrinth/assets'
import { ButtonStyled, OverflowMenu } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'

const props = defineProps<{
	signIn: () => void
}>()

const { friends, currentUser, loading, sendFriendRequest, respondFriendRequest, removeFriend } =
	useSocial()

const search = ref('')
const addOpen = ref(false)
const codeOrName = ref('')
const sending = ref(false)

const friendList = computed(() => friends.value?.friends ?? [])
const incoming = computed(() => friends.value?.incoming ?? [])

const filteredFriends = computed(() => {
	const q = search.value.trim().toLowerCase()
	if (!q) return friendList.value
	return friendList.value.filter((f) => (f.user?.username ?? '').toLowerCase().includes(q))
})

async function add() {
	const value = codeOrName.value.trim()
	if (!value) return
	sending.value = true
	const looksLikeCode = value.toUpperCase().startsWith('AMB-')
	await sendFriendRequest(looksLikeCode ? { friendCode: value } : { username: value })
	sending.value = false
	codeOrName.value = ''
	addOpen.value = false
}

async function accept(requestId: string) {
	await respondFriendRequest(requestId, true)
}

async function decline(requestId: string) {
	await respondFriendRequest(requestId, false)
}

async function unfriend(userId: string) {
	await removeFriend(userId)
}

function friendLabel(f: (typeof friendList.value)[number]) {
	return f.user?.displayName ?? f.user?.username ?? 'User'
}
</script>

<template>
	<div class="flex flex-col h-full">
		<div v-if="!currentUser" class="p-4 text-sm text-secondary">
			<ButtonStyled color="brand">
				<button class="w-full flex items-center justify-center gap-2" @click="props.signIn">
					<LogInIcon />
					Sign in to Amberite
				</button>
			</ButtonStyled>
			<p class="mt-2">Sign in to add friends and manage your friend group.</p>
		</div>
		<template v-else>
			<div class="flex items-center gap-2 p-2">
				<ButtonStyled circular type="transparent" @click="addOpen = true">
					<UserPlusIcon />
				</ButtonStyled>
				<input
					v-model="search"
					class="flex-1 rounded-lg bg-bg-input px-3 py-1.5 text-sm"
					placeholder="Search friends..."
				/>
				<ButtonStyled v-if="incoming.length" circular type="transparent">
					<div class="relative">
						<MailIcon />
						<span
							class="absolute -top-1 -right-1 bg-brand text-brand-inverted text-[8px] w-3 h-3 rounded-full flex items-center justify-center font-bold"
						>
							{{ incoming.length }}
						</span>
					</div>
				</ButtonStyled>
			</div>

			<div v-if="loading" class="p-2 space-y-2">
				<div v-for="n in 4" :key="n" class="flex gap-2 items-center animate-pulse">
					<div class="min-w-9 min-h-9 bg-button-bg rounded-full"></div>
					<div class="flex flex-col w-full">
						<div class="h-3 bg-button-bg rounded-full w-1/2 mb-1"></div>
						<div class="h-2.5 bg-button-bg rounded-full w-3/4"></div>
					</div>
				</div>
			</div>

			<div v-else class="flex-1 overflow-y-auto p-2 space-y-1">
				<div v-if="incoming.length" class="mb-2">
					<p class="text-xs font-semibold text-secondary mb-1">Requests</p>
					<div
						v-for="req in incoming"
						:key="req.request._id"
						class="flex items-center gap-2 p-1.5 rounded-lg hover:bg-button-bg transition-colors"
					>
						<img :src="req.user?.image ?? '/modrinth-app.svg'" class="w-8 h-8 rounded-full" />
						<div class="flex-1 min-w-0">
							<p class="text-sm truncate">
								{{ req.user?.displayName ?? req.user?.username ?? 'User' }}
							</p>
						</div>
						<div class="flex gap-1">
							<ButtonStyled size="small" color="brand" @click="accept(req.request._id)">
								Accept
							</ButtonStyled>
							<ButtonStyled size="small" @click="decline(req.request._id)">
								<XIcon />
							</ButtonStyled>
						</div>
					</div>
				</div>

				<div v-if="filteredFriends.length" class="mb-2">
					<p class="text-xs font-semibold text-secondary mb-1">
						Friends — {{ filteredFriends.length }}
					</p>
					<div
						v-for="f in filteredFriends"
						:key="f.friendshipId"
						class="group flex items-center gap-2 p-1.5 rounded-lg hover:bg-button-bg transition-colors"
					>
						<img :src="f.user?.image ?? '/modrinth-app.svg'" class="w-8 h-8 rounded-full" />
						<div class="flex-1 min-w-0">
							<p class="text-sm truncate">{{ friendLabel(f) }}</p>
						</div>
						<OverflowMenu
							class="opacity-0 group-hover:opacity-100 transition-opacity"
							:options="[
								{
									id: 'remove',
									action: () => f.user?.userId && unfriend(f.user.userId),
									color: 'danger',
								},
							]"
						>
							<template #remove> <XIcon /> Remove friend </template>
						</OverflowMenu>
					</div>
				</div>

				<div v-else-if="!search" class="text-sm text-secondary p-2">
					No friends yet.
					<button class="text-brand underline" @click="addOpen = true">Add a friend</button>
				</div>
			</div>
		</template>
	</div>

	<!-- Add friend modal -->
	<div
		v-if="addOpen"
		class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
		@click.self="addOpen = false"
	>
		<div class="bg-bg-raised rounded-2xl p-6 max-w-sm w-full mx-4 shadow-xl">
			<h3 class="text-lg font-bold mb-2">Add a friend</h3>
			<p class="text-sm text-secondary mb-4">Enter a friend code (AMB-XXXXXX) or a username.</p>
			<input
				v-model="codeOrName"
				class="w-full rounded-lg bg-bg-input px-3 py-2 text-sm mb-4"
				placeholder="AMB-ABC123 or username"
				@keyup.enter="add"
			/>
			<div class="flex gap-2 justify-end">
				<ButtonStyled @click="addOpen = false">Cancel</ButtonStyled>
				<ButtonStyled color="brand" :disabled="!codeOrName.trim() || sending" @click="add">
					{{ sending ? 'Sending…' : 'Add friend' }}
				</ButtonStyled>
			</div>
		</div>
	</div>
</template>
