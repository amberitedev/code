// What a checkout starts with: two friends, a Core the first one owns, and a modded instance the
// first shares with the second.

import { friends, instance, stack } from '@amberite/fixtures'

const { alice, bob } = await stack({
	apps: ['alice', 'bob'],
	cores: [{ owner: 'alice' }],
})

await friends(alice, bob)

await instance(alice, {
	name: 'Survival',
	loader: 'fabric',
	gameVersion: '1.21.1',
	mods: ['ferrite-core'],
	sharedWith: [bob],
})

await instance(bob, { name: 'Vanilla', gameVersion: '1.21.1' })
