import { expect } from 'e2e'

import { server, stack, test } from './amberite.ts'

test('a server page opened while its Core is off names the server', async ({
	amberite,
	app,
	screen,
}) => {
	const { alice, survival } = await amberite.seed(async () => {
		const { alice, cores } = await stack({ apps: ['alice'], cores: [{ owner: 'alice' }] })
		const survival = await server(cores[0]!, { name: 'Survival SMP', gameVersion: '1.21.1' })
		return { alice, survival }
	})

	// The App has shown this server before, as it has for anyone who made it.
	await amberite.open(alice, '/hosting/manage/')
	await screen.getByText('Survival SMP').waitFor()

	await amberite.stop(survival.core)
	await amberite.open(alice, `/hosting/manage/${survival.id}`)

	await app.screenshot('server-page-core-off')
	await expect(screen.getByText('Survival SMP')).toBeVisible()
})
