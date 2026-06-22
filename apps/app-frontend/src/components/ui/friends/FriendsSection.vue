<script setup lang="ts">
import { BanIcon, MoreVerticalIcon, PlayIcon, TrashIcon, UserIcon, UserPlusIcon, XIcon } from '@modrinth/assets'
import {
	Accordion,
	Avatar,
	ButtonStyled,
	defineMessages,
	OverflowMenu,
	useVIntl,
} from '@modrinth/ui'

import type { FriendWithUserData } from '@/helpers/friends.ts'

const { formatMessage } = useVIntl()

const props = withDefaults(
	defineProps<{
		friends: FriendWithUserData[]
		heading: string
		removeFriend: (friend: FriendWithUserData) => Promise<void>
		openProfile?: (friend: FriendWithUserData) => void
		inviteToGroup?: (friend: FriendWithUserData) => Promise<void>
		blockFriend?: (friend: FriendWithUserData) => Promise<void>
		unblockFriend?: (friend: FriendWithUserData) => Promise<void>
		inviteToPlay?: (friend: FriendWithUserData) => Promise<void>
		isSearching?: boolean
		openByDefault?: boolean
	}>(),
	{
		isSearching: false,
		openByDefault: false,
	},
)

function createOverflowMenuOptions(friend: FriendWithUserData) {
	if (props.unblockFriend) {
		return [
			{
				id: 'unblock-friend',
				action: () => props.unblockFriend?.(friend),
			},
		]
	}

	return [
		...(props.openProfile
			? [
					{
						id: 'view-profile',
						action: () => props.openProfile?.(friend),
					},
				]
		: []),
		...(props.inviteToGroup
			? [
					{
						id: 'invite-to-group',
						action: () => props.inviteToGroup?.(friend),
					},
				]
			: []),
		...(props.inviteToPlay
			? [
					{
						id: 'invite-to-play',
						action: () => props.inviteToPlay?.(friend),
					},
				]
			: []),
		{
			id: 'remove-friend',
			action: () => props.removeFriend(friend),
			color: 'red',
		},
		...(props.blockFriend
			? [
					{
						id: 'block-friend',
						action: () => props.blockFriend?.(friend),
						color: 'danger' as const,
						filled: true,
						hoverInvert: true,
					},
				]
			: []),
	]
}

function openFriendOverflowMenu(event: MouseEvent) {
	if (!(event.currentTarget instanceof HTMLElement)) return
	event.currentTarget.querySelector<HTMLButtonElement>('button')?.click()
}

const messages = defineMessages({
	removeFriend: {
		id: 'friends.friend.remove-friend',
		defaultMessage: 'Remove friend',
	},
	heading: {
		id: 'friends.section.heading',
		defaultMessage: '{title} - {count}',
	},
	friendRequestSent: {
		id: 'friends.friend.request-sent',
		defaultMessage: 'Friend request sent',
	},
	cancelRequest: {
		id: 'friends.friend.cancel-request',
		defaultMessage: 'Cancel request',
	},
	viewProfile: {
		id: 'friends.friend.view-profile',
		defaultMessage: 'View profile',
	},
	inviteToGroup: {
		id: 'friends.friend.invite-to-group',
		defaultMessage: 'Invite to friend group',
	},
	inviteToPlay: {
		id: 'friends.friend.invite-to-play',
		defaultMessage: 'Invite to play',
	},
	blockFriend: {
		id: 'friends.friend.block',
		defaultMessage: 'Block',
	},
	unblockFriend: {
		id: 'friends.friend.unblock',
		defaultMessage: 'Unblock',
	},
})
</script>

<template>
	<Accordion
		:open-by-default="openByDefault"
		:force-open="isSearching"
		:button-class="
			'flex w-full items-center bg-transparent border-0 p-0' +
			(isSearching
				? ''
				: ' cursor-pointer hover:brightness-[--hover-brightness] active:scale-[0.98] transition-all')
		"
	>
		<template #title>
			<h3 class="text-base text-primary font-medium m-0">
				{{ formatMessage(messages.heading, { title: heading, count: friends.length }) }}
			</h3>
		</template>
		<template #default>
			<div class="pt-3 flex flex-col gap-1">
				<div
					v-for="friend in friends"
					:key="friend.username"
					class="group grid items-center grid-cols-[auto_1fr_auto] gap-2 hover:bg-button-bg transition-colors rounded-full mr-1"
					@contextmenu.prevent.stop="openFriendOverflowMenu"
				>
					<div class="relative">
						<Avatar
							:src="friend.avatar"
							:class="{ grayscale: !friend.online && friend.accepted }"
							class="w-12 h-12 rounded-full"
							size="32px"
							circle
						/>
						<span
							v-if="friend.online"
							aria-hidden="true"
							class="bottom-[2px] right-[-2px] absolute w-3 h-3 bg-brand border-2 border-black border-solid rounded-full"
						/>
					</div>
					<div class="flex flex-col">
						<span
							class="text-sm m-0"
							:class="friend.online || !friend.accepted ? 'text-contrast' : 'text-primary'"
						>
							{{ friend.username }}
						</span>
						<span v-if="!friend.accepted" class="m-0 text-xs">
							{{ formatMessage(messages.friendRequestSent) }}
						</span>
						<span v-else-if="friend.status" class="m-0 text-xs">{{ friend.status }}</span>
					</div>
					<ButtonStyled v-if="friend.accepted" circular type="transparent">
						<OverflowMenu
							class="opacity-0 group-hover:opacity-100 transition-opacity"
							:options="createOverflowMenuOptions(friend)"
						>
							<MoreVerticalIcon />
							<template #view-profile>
								<UserIcon />
								{{ formatMessage(messages.viewProfile) }}
							</template>
							<template #invite-to-group>
								<UserPlusIcon />
								{{ formatMessage(messages.inviteToGroup) }}
							</template>
							<template #invite-to-play>
								<PlayIcon />
								{{ formatMessage(messages.inviteToPlay) }}
							</template>
							<template #block-friend>
								<BanIcon />
								{{ formatMessage(messages.blockFriend) }}
							</template>
							<template #unblock-friend>
								<BanIcon />
								{{ formatMessage(messages.unblockFriend) }}
							</template>
							<template #remove-friend>
								<TrashIcon />
								{{ formatMessage(messages.removeFriend) }}
							</template>
						</OverflowMenu>
					</ButtonStyled>
					<ButtonStyled v-else type="transparent" circular>
						<button v-tooltip="formatMessage(messages.cancelRequest)" @click="removeFriend(friend)">
							<XIcon />
						</button>
					</ButtonStyled>
				</div>
			</div>
		</template>
	</Accordion>
</template>
