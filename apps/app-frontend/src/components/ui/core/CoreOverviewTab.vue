<script setup>
import { CheckIcon, CopyIcon, LinkIcon, ShieldIcon, UsersIcon } from '@modrinth/assets'
import { ButtonStyled, StyledInput } from '@modrinth/ui'

defineProps({
	membersCount: { type: Number, required: true },
	localMembersCount: { type: Number, required: true },
	inviteCode: { type: String, default: null },
})

defineEmits(['create-invite', 'save-core'])

const core = defineModel('core', { type: Object, default: null })
</script>

<template>
	<section class="grid gap-4 md:grid-cols-2">
		<div
			class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5 md:col-span-2"
		>
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<ShieldIcon /> Core identity
			</h2>
			<div class="grid gap-3 md:grid-cols-2">
				<StyledInput
					v-if="core"
					v-model="core.name"
					placeholder="Core name"
					wrapper-class="w-full"
				/>
				<StyledInput
					v-if="core"
					v-model="core.subdomain"
					placeholder="name.amberite.dev"
					wrapper-class="w-full"
				/>
				<StyledInput
					v-if="core"
					v-model="core.description"
					placeholder="Friend group description"
					wrapper-class="w-full md:col-span-2"
				/>
			</div>
			<ButtonStyled color="brand">
				<button class="mt-4" @click="$emit('save-core')"><CheckIcon /> Save identity</button>
			</ButtonStyled>
		</div>

		<div class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5">
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<UsersIcon /> Group
			</h2>
			<p>{{ membersCount }} Convex members</p>
			<p>{{ localMembersCount }} local permission records</p>
		</div>

		<div class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5">
			<h2 class="m-0 mb-3 flex items-center gap-2 text-xl font-black text-contrast">
				<LinkIcon /> Invite link
			</h2>
			<ButtonStyled>
				<button @click="$emit('create-invite')"><CopyIcon /> Create invite</button>
			</ButtonStyled>
			<p v-if="inviteCode">amberite.dev/invite/{{ inviteCode }}</p>
		</div>
	</section>
</template>
