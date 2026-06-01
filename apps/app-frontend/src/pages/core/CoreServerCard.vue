<script setup lang="ts">
import type {
	ConvexSyncedProfile,
	ProfileWhitelistResult,
	SyncedProfileSettings,
} from '@amberite/amberite-api'
import { ButtonStyled } from '@modrinth/ui'
import { reactive, ref } from 'vue'

defineOptions({ name: 'CoreServerCard' })

const props = defineProps<{
	profile: ConvexSyncedProfile
	members: {
		userId: string
		role: string
		user?: { displayName?: string; username?: string } | null
	}[]
	save: (profileId: string, settings: SyncedProfileSettings) => Promise<void>
	loadWhitelist: (profileId: string) => Promise<ProfileWhitelistResult | null>
}>()

const selectableRoles = ['admin', 'member'] as const

const form = reactive<SyncedProfileSettings>({
	visibility: props.profile.visibility ?? 'everyone',
	visibilityRoles: [...(props.profile.visibilityRoles ?? ['admin', 'member'])],
	visibilityUserIds: [...(props.profile.visibilityUserIds ?? [])],
	autoWhitelist: props.profile.autoWhitelist ?? false,
	whitelistScope: props.profile.whitelistScope ?? 'viewers',
	whitelistRoles: [...(props.profile.whitelistRoles ?? ['admin', 'member'])],
	whitelistUserIds: [...(props.profile.whitelistUserIds ?? [])],
})

const saving = ref(false)
const savedNote = ref(false)
const whitelist = ref<ProfileWhitelistResult | null>(null)

function toggle(list: string[] | undefined, value: string): string[] {
	const set = new Set(list ?? [])
	if (set.has(value)) set.delete(value)
	else set.add(value)
	return [...set]
}

async function onSave() {
	saving.value = true
	savedNote.value = false
	await props.save(props.profile._id, { ...form })
	saving.value = false
	savedNote.value = true
}

async function preview() {
	whitelist.value = await props.loadWhitelist(props.profile._id)
}

function memberLabel(m: (typeof props.members)[number]) {
	return m.user?.displayName ?? m.user?.username ?? m.userId
}
</script>

<template>
	<div class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3">
		<div class="flex items-center justify-between">
			<span class="font-bold">{{ profile.name }}</span>
			<span class="text-secondary text-xs">{{ profile.gameVersion }} {{ profile.loader }}</span>
		</div>

		<div v-if="!profile.viewerCanManage" class="text-secondary text-sm">
			Visibility: {{ profile.visibility ?? 'everyone' }} · managed by an admin.
		</div>

		<template v-else>
			<div class="flex flex-col gap-2">
				<span class="font-semibold text-sm">Who can see this server</span>
				<label class="flex items-center gap-2">
					<input v-model="form.visibility" type="radio" value="everyone" /> Everyone in the group
				</label>
				<label class="flex items-center gap-2">
					<input v-model="form.visibility" type="radio" value="roles" /> Specific roles
				</label>
				<div v-if="form.visibility === 'roles'" class="flex gap-3 pl-6">
					<label
						v-for="role in selectableRoles"
						:key="role"
						class="flex items-center gap-1 text-sm"
					>
						<input
							type="checkbox"
							:checked="form.visibilityRoles?.includes(role)"
							@change="form.visibilityRoles = toggle(form.visibilityRoles, role)"
						/>
						{{ role }}
					</label>
				</div>
				<label class="flex items-center gap-2">
					<input v-model="form.visibility" type="radio" value="custom" /> Only selected people
				</label>
				<div v-if="form.visibility === 'custom'" class="flex flex-col gap-1 pl-6">
					<label v-for="m in members" :key="m.userId" class="flex items-center gap-1 text-sm">
						<input
							type="checkbox"
							:checked="form.visibilityUserIds?.includes(m.userId)"
							@change="form.visibilityUserIds = toggle(form.visibilityUserIds, m.userId)"
						/>
						{{ memberLabel(m) }} <span class="text-secondary">({{ m.role }})</span>
					</label>
				</div>
			</div>

			<div class="flex flex-col gap-2 border-t border-surface-5 pt-3">
				<label class="flex items-center gap-2 font-semibold text-sm">
					<input v-model="form.autoWhitelist" type="checkbox" /> Automatic whitelist
				</label>
				<div v-if="form.autoWhitelist" class="flex flex-col gap-2 pl-6">
					<span class="text-secondary text-sm">Whitelist which members' Minecraft accounts?</span>
					<label class="flex items-center gap-2 text-sm">
						<input v-model="form.whitelistScope" type="radio" value="viewers" /> Everyone who can
						see it
					</label>
					<label class="flex items-center gap-2 text-sm">
						<input v-model="form.whitelistScope" type="radio" value="roles" /> Specific roles
					</label>
					<div v-if="form.whitelistScope === 'roles'" class="flex gap-3 pl-6">
						<label
							v-for="role in selectableRoles"
							:key="role"
							class="flex items-center gap-1 text-sm"
						>
							<input
								type="checkbox"
								:checked="form.whitelistRoles?.includes(role)"
								@change="form.whitelistRoles = toggle(form.whitelistRoles, role)"
							/>
							{{ role }}
						</label>
					</div>
					<label class="flex items-center gap-2 text-sm">
						<input v-model="form.whitelistScope" type="radio" value="custom" /> Only selected people
					</label>
					<div v-if="form.whitelistScope === 'custom'" class="flex flex-col gap-1 pl-6">
						<label v-for="m in members" :key="m.userId" class="flex items-center gap-1 text-sm">
							<input
								type="checkbox"
								:checked="form.whitelistUserIds?.includes(m.userId)"
								@change="form.whitelistUserIds = toggle(form.whitelistUserIds, m.userId)"
							/>
							{{ memberLabel(m) }}
						</label>
					</div>
				</div>
			</div>

			<div class="flex items-center gap-3">
				<ButtonStyled color="brand">
					<button :disabled="saving" @click="onSave">{{ saving ? 'Saving…' : 'Save' }}</button>
				</ButtonStyled>
				<ButtonStyled>
					<button @click="preview">Preview whitelist</button>
				</ButtonStyled>
				<span v-if="savedNote" class="text-green text-sm">Saved.</span>
			</div>

			<div v-if="whitelist" class="text-sm text-secondary flex flex-col gap-1">
				<span class="font-semibold">Resolved whitelist ({{ whitelist.entries.length }})</span>
				<span v-for="entry in whitelist.entries" :key="entry.userId">
					{{ entry.displayName ?? entry.userId }} —
					{{
						entry.accounts
							.map((a) => a.gamertag ?? a.minecraftUuid ?? '(no linked account)')
							.join(', ') || 'no linked Minecraft account'
					}}
				</span>
				<span v-if="whitelist.entries.length === 0">No eligible members.</span>
			</div>
		</template>
	</div>
</template>
