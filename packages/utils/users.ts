// noinspection JSUnusedGlobalSymbols

export const getUserLink = (user: { username: string }) => {
	return `/user/${user.username}`
}

export const isStaff = (user: { role: string } | null | undefined) => {
	return !!user && STAFF_ROLES.includes(user.role)
}

export const isAdmin = (user: { role: string } | null | undefined) => {
	return user && user.role === 'admin'
}

export const STAFF_ROLES = ['moderator', 'admin']

export const MODRINTH_USER_ID = '2REoufqX'
export const AUTOMOD_USER_ID = ''
export const MODRINTH_ARCHIVES_USER_ID = 'GVFjtWTf'

export const OFFICIAL_ACCOUNT_IDS = [MODRINTH_USER_ID, AUTOMOD_USER_ID, MODRINTH_ARCHIVES_USER_ID]

export const isModrinthUser = (userId: string | undefined) => {
	return userId === MODRINTH_USER_ID
}

export const isOfficialAccount = (userId: string | undefined) => {
	return userId !== undefined && OFFICIAL_ACCOUNT_IDS.includes(userId)
}
