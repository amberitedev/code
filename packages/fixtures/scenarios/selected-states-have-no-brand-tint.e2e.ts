import { expect } from 'e2e'

import { instance, stack, test } from './amberite.ts'

// Counts elements whose background is a translucent green or orange: over the dark theme an
// orange tint turns brown, so selected states use a neutral lift and keep orange for text.
const BRAND_TINTS = `(() => {
	const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true })
	let count = 0
	for (const el of document.querySelectorAll('*')) {
		const bg = getComputedStyle(el).backgroundColor
		if (!bg || bg === 'rgba(0, 0, 0, 0)' || bg === 'transparent') continue
		ctx.clearRect(0, 0, 1, 1)
		ctx.fillStyle = bg
		ctx.fillRect(0, 0, 1, 1)
		const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data
		const green = g > r + 15 && g > b + 10
		const orange = r > 180 && g < 170 && b < 110
		if (a > 0 && a < 250 && (green || orange)) count++
	}
	return count
})()`

test('selected states in the dark theme have no brand tint', async ({
	amberite,
	app,
	browser,
	screen,
}) => {
	const { alice, bob } = await amberite.seed(async () => {
		const { alice, bob } = await stack({ apps: ['alice', 'bob'], cores: [{ owner: 'alice' }] })
		await instance(alice, { name: 'Survival', loader: 'fabric', gameVersion: '1.21.1' })
		return { alice, bob }
	})
	let tints = 0

	await amberite.open(bob, '/')
	await screen.getByText('Create an instance').waitFor()
	await app.screenshot('welcome')
	tints += (await browser.evaluate<number>(BRAND_TINTS)) ?? 0

	await amberite.open(alice, '/')
	await screen.getByRole('button', { name: /Last played/i }).click()
	await screen.getByRole('option', { name: 'Last played' }).waitFor()
	await app.screenshot('library-sort')
	tints += (await browser.evaluate<number>(BRAND_TINTS)) ?? 0
	await screen.getByRole('option', { name: 'Last played' }).click()

	await screen.getByRole('button', { name: 'Settings' }).click()
	await screen.getByRole('button', { name: 'Appearance' }).click()
	await screen.getByText('Color theme').waitFor()
	await app.screenshot('settings')
	tints += (await browser.evaluate<number>(BRAND_TINTS)) ?? 0

	expect(tints).toBe(0)
})
