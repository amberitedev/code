// Upstream's pages read our backend's social routes through their labrinth modules and types. This
// file fails to compile when a generated shape stops fitting the labrinth type it stands in for.
import type {
	AllProjects,
	PreferencesUpdate,
	SearchUser,
	User,
	UserFriend,
	UserPreferences,
	UserUpdate,
} from '@amberite/contracts'

import type { Labrinth } from '../modules/labrinth/types'

/**
 * Whether every value of `Ours` reads as a `Theirs`. Labrinth sends null where upstream's types
 * say a field is optional, so a null fits wherever `Theirs` allows the field to be absent.
 */
type Fits<Ours, Theirs> = [Ours] extends [Theirs]
	? true
	: null extends Ours
		? undefined extends Theirs
			? Fits<Exclude<Ours, null>, Theirs>
			: false
		: [Ours, Theirs] extends [readonly (infer O)[], readonly (infer T)[]]
			? Fits<O, T>
			: [Ours, Theirs] extends [object, object]
				? false extends {
						[K in keyof Theirs]: K extends keyof Ours
							? Fits<Ours[K], Theirs[K]>
							: undefined extends Theirs[K]
								? true
								: false
					}[keyof Theirs]
					? false
					: true
				: false

type Assert<T extends true> = T

export type LabrinthCompat = [
	Assert<Fits<User, Labrinth.Users.v3.User>>,
	Assert<Fits<User, Labrinth.Users.v2.User>>,
	Assert<Fits<SearchUser, Labrinth.Users.v3.SearchUser>>,
	Assert<Fits<UserFriend, Labrinth.Friends.v3.UserFriend>>,
	Assert<Fits<AllProjects, Labrinth.Users.v3.AllProjectsResponse>>,
	Assert<Fits<UserPreferences, Labrinth.Users.v3.UserPreferences>>,
	// Request bodies go the other way: what upstream sends must fit what the backend takes.
	Assert<Fits<Labrinth.Users.v3.PartialUserPreferences, PreferencesUpdate>>,
	Assert<Fits<Labrinth.Users.v3.EditUserRequest, UserUpdate>>,
	// @ts-expect-error A field of another type does not fit.
	Assert<Fits<{ id: number }, { id: string }>>,
	// @ts-expect-error A null does not fit a required field.
	Assert<Fits<{ id: string | null }, { id: string }>>,
]
