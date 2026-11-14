// Upstream's pages read our backend's account, social and shared instance routes through their
// labrinth and sharedinstances modules and types. This file fails to compile when a generated
// shape stops fitting the upstream type it stands in for.
import type {
	AllProjects,
	AuthSession,
	BlacklistStatus,
	Globals,
	InstanceUsers,
	InstanceVersion,
	Invite,
	InviteLink,
	LoginAnswer,
	LoginCredentials,
	NewAccount,
	PasswordReset,
	PasswordUpdate,
	PreferencesUpdate,
	SearchUser,
	SharedInstance,
	TwoFactorLogin,
	User,
	UserFriend,
	UserIds,
	UserPreferences,
	UserSession,
	UserUpdate,
} from '@amberite/contracts'

import type { Labrinth } from '../modules/labrinth/types'
import type { SharedInstances } from '../modules/shared-instances/types'

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
	Assert<Fits<Globals, Labrinth.Globals.Internal.Globals>>,
	Assert<Fits<LoginAnswer, Labrinth.Auth.v2.LoginResponse>>,
	Assert<Fits<AuthSession, Labrinth.Auth.v2.AuthSessionResponse>>,
	Assert<Fits<UserSession, Labrinth.Sessions.v2.Session>>,
	Assert<Fits<SharedInstance, SharedInstances.Instances.v1.Instance>>,
	Assert<Fits<InstanceUsers, SharedInstances.Instances.v1.InstanceUsers>>,
	Assert<Fits<InstanceVersion, SharedInstances.Instances.v1.InstanceVersion>>,
	Assert<Fits<Invite, SharedInstances.Invites.v1.Invite>>,
	Assert<Fits<InviteLink, SharedInstances.Invites.v1.InviteLink>>,
	Assert<Fits<BlacklistStatus, SharedInstances.Users.v1.BlacklistStatus>>,
	// Request bodies go the other way: what upstream sends must fit what the backend takes.
	Assert<Fits<Labrinth.Users.v3.PartialUserPreferences, PreferencesUpdate>>,
	Assert<Fits<Labrinth.Users.v3.EditUserRequest, UserUpdate>>,
	Assert<Fits<Labrinth.Auth.v2.CreateAccountRequest, NewAccount>>,
	Assert<Fits<Labrinth.Auth.v2.ValidateCreateAccountRequest, NewAccount>>,
	Assert<Fits<Labrinth.Auth.v2.LoginRequest, LoginCredentials>>,
	Assert<Fits<Labrinth.Auth.v2.Login2FARequest, TwoFactorLogin>>,
	Assert<Fits<Labrinth.Auth.v2.ResetPasswordRequest, PasswordReset>>,
	Assert<Fits<Labrinth.Auth.v2.ChangePasswordRequest, PasswordUpdate>>,
	Assert<Fits<SharedInstances.Moderation.v1.BlacklistUserRequest, UserIds>>,
	// @ts-expect-error A field of another type does not fit.
	Assert<Fits<{ id: number }, { id: string }>>,
	// @ts-expect-error A null does not fit a required field.
	Assert<Fits<{ id: string | null }, { id: string }>>,
]
