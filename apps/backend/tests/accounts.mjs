import assert from 'node:assert/strict'
import { createHmac, randomBytes } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../../', import.meta.url))
const statePath = path.join(root, '.data/backend-proof/accounts.json')
const base = process.argv[2] ?? process.env.BACKEND_PROOF_URL
assert(
	base && ['127.0.0.1', 'localhost', '[::1]'].includes(new URL(base).hostname),
	'Pass the running local backend URL',
)

async function request(route, { token, method = 'GET', body, expected = 200 } = {}) {
	const response = await fetch(new URL(route, base), {
		method,
		headers: {
			...(token ? { Authorization: token } : {}),
			...(body ? { 'Content-Type': 'application/json' } : {}),
		},
		body: body ? JSON.stringify(body) : undefined,
	})
	const text = await response.text()
	assert.equal(response.status, expected, `${method} ${route}: ${text}`)
	return text ? JSON.parse(text) : null
}

const globals = await request('/_internal/globals')
assert.equal(globals.captcha_enabled, false)
assert.deepEqual(globals.tax_compliance_thresholds, { 2025: 600, 2026: 2000 })

let accounts
try {
	accounts = JSON.parse(await readFile(statePath, 'utf8'))
} catch (error) {
	if (error.code !== 'ENOENT') throw error
	const suffix = randomBytes(5).toString('hex')
	accounts = ['owner', 'recipient', 'stranger'].map((role) => ({
		username: `proof_${role}_${suffix}`,
		email: `${role}.${suffix}@example.test`,
		password: randomBytes(24).toString('base64url'),
	}))
	for (const account of accounts) {
		const session = await request('/v2/auth/create', {
			method: 'POST',
			body: { ...account, challenge: '', account_consent: true },
		})
		account.id = session.user_id
	}
	await mkdir(path.dirname(statePath), { recursive: true })
	await writeFile(statePath, JSON.stringify(accounts, null, 2))
}

for (const account of accounts) {
	const session = await request('/v2/auth/login', {
		method: 'POST',
		body: { username: account.email, password: account.password, challenge: '' },
	})
	assert.equal(session.user_id, account.id, 'Password login retained persistent account ID')
	account.token = session.session
}
const [owner, recipient, stranger] = accounts
await request('/v2/auth/login', {
	method: 'POST',
	body: { username: owner.email, password: 'incorrect-password', challenge: '' },
	expected: 401,
})
await request('/v2/user', { expected: 401 })
assert.equal((await request('/v2/user', { token: owner.token })).id, owner.id)
assert.equal((await request('/v2/user', { token: recipient.token })).id, recipient.id)
assert.equal((await request(`/v3/user/${owner.id}`, { token: recipient.token })).email, null)
await request(`/v3/user/${owner.id}`, {
	method: 'PATCH',
	token: recipient.token,
	body: { bio: 'forbidden' },
	expected: 401,
})
await request(`/v3/user/${owner.id}`, {
	method: 'PATCH',
	token: owner.token,
	body: { role: 'admin' },
	expected: 403,
})
console.log('PASS persistent password accounts, isolation, profile permissions')

await request(`/v3/friend/${recipient.id}`, { method: 'DELETE', token: owner.token, expected: 204 })
await request(`/v3/friend/${owner.id}`, { method: 'POST', token: owner.token, expected: 400 })
await request(`/v3/friend/${recipient.id}`, { method: 'POST', token: owner.token, expected: 204 })
await request(`/v3/friend/${recipient.id}`, { method: 'POST', token: owner.token, expected: 400 })
const pending = await request('/v3/friends', { token: recipient.token })
assert(
	pending.some(
		(friend) => friend.id === recipient.id && friend.friend_id === owner.id && !friend.accepted,
	),
)
await request(`/v3/friend/${owner.id}`, { method: 'POST', token: recipient.token, expected: 204 })
assert(
	(await request('/v3/friends', { token: owner.token })).some(
		(friend) => friend.id === recipient.id && friend.accepted,
	),
)
assert.deepEqual(await request('/v3/friends', { token: stranger.token }), [])
await request(`/v3/block/${owner.id}`, { method: 'POST', token: recipient.token, expected: 204 })
assert.deepEqual(await request('/v3/friends', { token: owner.token }), [])
await request(`/v3/friend/${recipient.id}`, { method: 'POST', token: owner.token, expected: 400 })
await request(`/v3/block/${owner.id}`, { method: 'DELETE', token: recipient.token, expected: 204 })
await request(`/v3/friend/${recipient.id}`, { method: 'POST', token: owner.token, expected: 204 })
await request(`/v3/friend/${owner.id}`, { method: 'POST', token: recipient.token, expected: 204 })
console.log('PASS friend invitation, acceptance, removal by blocking, unblock')

const oldToken = owner.token
const rotations = await Promise.all([
	fetch(new URL('/v2/session/refresh', base), {
		method: 'POST',
		headers: { Authorization: oldToken },
	}),
	fetch(new URL('/v2/session/refresh', base), {
		method: 'POST',
		headers: { Authorization: oldToken },
	}),
])
assert.deepEqual(
	rotations.map((response) => response.status).sort(),
	[200, 401],
	'Only one concurrent rotation succeeds',
)
owner.token = (await rotations.find((response) => response.ok).json()).session
await request('/v2/user', { token: oldToken, expected: 401 })
const sessions = await request('/v2/session/list', { token: owner.token })
assert.equal(sessions.filter((session) => session.current).length, 1)
assert(
	sessions.every((session) => session.session === null),
	'Session list must not expose secrets',
)
const extra = await request('/v2/auth/login', {
	method: 'POST',
	body: { username: owner.username, password: owner.password, challenge: '' },
})
await request(`/v2/session/${extra.id}`, {
	method: 'DELETE',
	token: recipient.token,
	expected: 204,
})
await request('/v2/user', { token: extra.session })
await request(`/v2/session/${extra.id}`, { method: 'DELETE', token: owner.token, expected: 204 })
await request('/v2/user', { token: extra.session, expected: 401 })
console.log('PASS atomic session refresh, replay rejection, authorized session revocation')

async function connect(token) {
	const url = new URL('/_internal/launcher_socket', base)
	url.protocol = 'ws:'
	url.searchParams.set('code', token)
	const socket = new WebSocket(url)
	const messages = []
	socket.addEventListener('message', (event) => messages.push(JSON.parse(event.data)))
	await new Promise((resolve, reject) => {
		socket.addEventListener('open', resolve, { once: true })
		socket.addEventListener('error', () => reject(new Error('Friends socket failed to open')), {
			once: true,
		})
	})
	return {
		socket,
		async receive(predicate) {
			const end = Date.now() + 5000
			while (Date.now() < end) {
				const index = messages.findIndex(predicate)
				if (index >= 0) return messages.splice(index, 1)[0]
				await new Promise((resolve) => setTimeout(resolve, 25))
			}
			throw new Error('Friends socket message timed out')
		},
	}
}
const ownerSocket = await connect(owner.token)
const recipientSocket = await connect(recipient.token)
try {
	const sync = await recipientSocket.receive((message) => message.type === 'friend_statuses')
	assert(sync.statuses.some((status) => status.user_id === owner.id))
	ownerSocket.socket.send(
		JSON.stringify({ type: 'status_update', profile_name: 'Account proof instance' }),
	)
	await recipientSocket.receive(
		(message) =>
			message.type === 'status_update' && message.status.profile_name === 'Account proof instance',
	)
	ownerSocket.socket.close()
	await recipientSocket.receive(
		(message) => message.type === 'user_offline' && message.id === owner.id,
	)
	console.log('PASS native friends WebSocket initial sync, presence, disconnect')
} finally {
	ownerSocket.socket.close()
	recipientSocket.socket.close()
}

const setup = await request('/v2/auth/2fa/get_secret', { method: 'POST', token: owner.token })
let accumulator = 0,
	bits = 0
const decoded = []
for (const char of setup.secret) {
	accumulator = (accumulator << 5) | 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'.indexOf(char)
	bits += 5
	if (bits >= 8) {
		bits -= 8
		decoded.push((accumulator >> bits) & 255)
	}
}
const counter = Buffer.alloc(8)
counter.writeBigUInt64BE(BigInt(Math.floor(Date.now() / 30000)))
const hmac = createHmac('sha1', Buffer.from(decoded)).update(counter).digest()
const code = String((hmac.readUInt32BE(hmac[19] & 15) & 0x7fffffff) % 1000000).padStart(6, '0')
const { backup_codes: backup } = await request('/v2/auth/2fa', {
	method: 'POST',
	token: owner.token,
	body: { flow: setup.flow, code },
})
try {
	const login = await request('/v2/auth/login', {
		method: 'POST',
		body: { username: owner.username, password: owner.password, challenge: '' },
	})
	assert.equal(login.error, '2fa_required')
	const session = await request('/v2/auth/login/2fa', {
		method: 'POST',
		body: { flow: login.flow, code: backup[0] },
	})
	assert.equal(session.user_id, owner.id)
	await request('/v2/auth/login/2fa', {
		method: 'POST',
		body: { flow: login.flow, code: backup[0] },
		expected: 401,
	})
	await request(`/v3/user/${owner.id}/notifications`, { token: recipient.token, expected: 401 })
	const notifications = await request(`/v3/user/${owner.id}/notifications`, { token: owner.token })
	assert(notifications.some((notification) => notification.body.type === 'two_factor_enabled'))
	console.log('PASS TOTP enrollment, backup login, one-use auth flow, private notifications')
} finally {
	await request('/v2/auth/2fa', {
		method: 'DELETE',
		token: owner.token,
		body: { code: backup[1] },
		expected: 204,
	})
}
console.log(
	'Account proof passed. Rerun after backend restart to verify the same database accounts persist.',
)
