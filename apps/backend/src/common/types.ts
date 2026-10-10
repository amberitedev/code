export interface Env extends Cloudflare.Env {
	LOCAL_DEV?: string
	LOCAL_DEV_SECRET?: string
	CORS_ORIGINS?: string
}

export interface UserRow {
	id: string
	username: string
	email: string
	password_hash: string | null
	email_verified: number
	totp_secret: string | null
	avatar_url: string | null
	bio: string | null
	created: string
	allow_friend_requests: number
	is_dev: number
	role: 'developer' | 'moderator' | 'admin'
	preferences: string
}
