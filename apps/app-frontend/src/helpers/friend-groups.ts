import type {
	AmberiteUser,
	FriendGroupInvite,
	FriendGroupMember,
	FriendGroupSummary,
} from '@amberite/amberite-api'
import { ConvexApiClient } from '@amberite/amberite-api'

import { getDesktopAdapter } from '@/adapters/desktop'

import { ensureAmberiteSession } from './amberite-auth'

export type { AmberiteUser, FriendGroupInvite, FriendGroupMember, FriendGroupSummary }

async function client(): Promise<ConvexApiClient> {
	await ensureAmberiteSession()
	return new ConvexApiClient(getDesktopAdapter())
}

export const currentUser = async () => (await client()).currentUser()
export const ensureSocialProfile = async () => (await client()).ensureSocialProfile()
export const listMyFriendGroups = async () => (await client()).listMyFriendGroups()
export const listFriendGroupMembers = async (friendGroupId: string) =>
	(await client()).listFriendGroupMembers(friendGroupId)
export const updateFriendGroup = async (
	args: Parameters<ConvexApiClient['updateFriendGroup']>[0],
) => (await client()).updateFriendGroup(args)
export const ensureCoreFriendGroup = async (
	args: Parameters<ConvexApiClient['ensureCoreFriendGroup']>[0],
) => (await client()).ensureCoreFriendGroup(args)
export const claimPairingCore = async (args: { code: string }) =>
	(await client()).claimPairingCore(args.code)
export const createFriendGroupInvite = async (
	args: Parameters<ConvexApiClient['createFriendGroupInvite']>[0],
) => (await client()).createFriendGroupInvite(args)
export const updateMemberRole = async (args: Parameters<ConvexApiClient['updateMemberRole']>[0]) =>
	(await client()).updateMemberRole(args)
export const sendFriendRequest = async (
	args: Parameters<ConvexApiClient['sendFriendRequest']>[0],
) => (await client()).sendFriendRequest(args)

export function friendGroupId(group: FriendGroupSummary): string {
	return group.group.id ?? group.group._id ?? ''
}
