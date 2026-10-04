const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'

export function randomId(length = 8): string {
	let result = ''
	while (result.length < length) {
		for (const byte of crypto.getRandomValues(new Uint8Array(length))) {
			if (byte < 248 && result.length < length) result += alphabet[byte % 62]
		}
	}
	return result
}

export async function digest(value: string): Promise<string> {
	const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))
	return Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join('')
}

async function derivePassword(password: string, salt: string): Promise<string> {
	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(password),
		'PBKDF2',
		false,
		['deriveBits'],
	)
	const bits = await crypto.subtle.deriveBits(
		{ name: 'PBKDF2', hash: 'SHA-256', salt: new TextEncoder().encode(salt), iterations: 100000 },
		key,
		256,
	)
	return Array.from(new Uint8Array(bits), (byte) => byte.toString(16).padStart(2, '0')).join('')
}

// Workers Web Crypto supplies PBKDF2. Existing Modrinth password hashes are not imported.
export async function hashPassword(password: string): Promise<string> {
	const salt = randomId(32)
	return `pbkdf2-sha256$100000$${salt}$${await derivePassword(password, salt)}`
}

export async function checkPassword(password: string, stored: string | null): Promise<boolean> {
	const parts = stored?.split('$')
	const actual = await derivePassword(password, parts?.[2] ?? 'invalid-account-password-salt')
	const expected = parts?.[3] ?? ''.padEnd(64, '0')
	let difference = actual.length ^ expected.length
	for (let i = 0; i < actual.length; i++)
		difference |= actual.charCodeAt(i) ^ expected.charCodeAt(i)
	return Boolean(stored) && difference === 0
}

export function totpSecret(): string {
	const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'
	return Array.from(crypto.getRandomValues(new Uint8Array(32)), (byte) => chars[byte & 31]).join('')
}

export async function totpCounter(secret: string, code: string): Promise<number | null> {
	if (!/^\d{6}$/.test(code)) return null
	const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'
	let bits = 0,
		accumulator = 0
	const decoded: number[] = []
	for (const char of secret) {
		accumulator = (accumulator << 5) | chars.indexOf(char)
		bits += 5
		if (bits >= 8) {
			bits -= 8
			decoded.push((accumulator >> bits) & 255)
		}
	}
	const key = await crypto.subtle.importKey(
		'raw',
		new Uint8Array(decoded),
		{ name: 'HMAC', hash: 'SHA-1' },
		false,
		['sign'],
	)
	const current = Math.floor(Date.now() / 30000)
	for (const counter of [current - 1, current, current + 1]) {
		const input = new ArrayBuffer(8)
		new DataView(input).setBigUint64(0, BigInt(counter))
		const hash = new Uint8Array(await crypto.subtle.sign('HMAC', key, input))
		const offset = hash[19] & 15
		const value =
			((hash[offset] & 127) << 24) |
			(hash[offset + 1] << 16) |
			(hash[offset + 2] << 8) |
			hash[offset + 3]
		if (String(value % 1000000).padStart(6, '0') === code) return counter
	}
	return null
}
