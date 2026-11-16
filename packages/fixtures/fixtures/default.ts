// What a checkout starts with: two friends, a Core the first one owns, and a modded instance the
// first shares with the second.

import { friends, instance, stack } from '@amberite/fixtures'

const { alice, bob } = await stack({
	apps: ['alice', 'bob'],
	cores: [{ owner: 'alice' }],
})

await friends(alice, bob)

// Every checkout's first start applies this, so lines that do not depend on each other run together.
await Promise.all([
	instance(alice, {
		name: 'Survival',
		loader: 'fabric',
		gameVersion: '1.21.1',
		mods: ['ferrite-core'],
		sharedWith: [bob],
	}),
	instance(bob, { name: 'Vanilla', gameVersion: '1.21.1' }),
])
