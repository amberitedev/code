/**
 * Mocked friends helper - returns empty/fake data
 * TODO: connect to backend API - replace with real friends list
 */

import type { User } from '@modrinth/utils'
import type { Dayjs } from 'dayjs'
import dayjs from 'dayjs'

import { get_user_many } from '@/helpers/cache'
import type { ModrinthCredentials } from '@/helpers/mr_auth'

export type UserStatus = {
	user_id: string
	profile_name: string | null
	last_update: string
}

export type UserFriend = {
	id: string
	friend_id: string
	accepted: boolean
	created: string
}

export async function friends(): Promise<UserFriend[]> {
	return []
}

export async function friend_statuses(): Promise<UserStatus[]> {
	return []
}

export async function add_friend(_userId: string): Promise<void> {
	// no-op
}

export async function remove_friend(_userId: string): Promise<void> {
	// no-op
}

export type FriendWithUserData = {
	id: string
	friend_id: string | null
	status: string | null
	last_updated: Dayjs | null
	created: Dayjs
	username: string
	accepted: boolean
	online: boolean
	avatar: string
}

export async function transformFriends(
	_friends: UserFriend[],
	_credentials: ModrinthCredentials | null,
): Promise<FriendWithUserData[]> {
	return []
}
