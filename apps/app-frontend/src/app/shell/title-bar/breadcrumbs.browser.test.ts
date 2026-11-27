import { expect, test } from 'vite-plus/test'
import { render } from 'vitest-browser-vue'
import { defineComponent, h } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import { provideBreadcrumbManager } from '@/shared/breadcrumbs'
import { createBreadcrumbManager } from '@/shared/breadcrumbs/create-breadcrumb-manager'

import Breadcrumbs from './breadcrumbs.vue'

/** The title bar's breadcrumbs, in a bar `width` pixels wide, on a server called `name`. */
function titleBar(width: number, name: string) {
	const manager = createBreadcrumbManager()
	const servers = manager.reset({ slot: 'root', id: 'servers', label: 'Servers', to: '/' })
	manager.push({ slot: 'server', id: 'server', label: name }, { parent: servers })
	const container = document.body.appendChild(document.createElement('div'))
	container.style.cssText = `display: flex; width: ${width}px`
	const router = createRouter({
		history: createMemoryHistory(),
		routes: [{ path: '/', component: {} }],
	})
	const bar = defineComponent(() => {
		provideBreadcrumbManager(manager)
		return () => h(Breadcrumbs)
	})
	return render(bar, { container, global: { plugins: [router] } })
}

test('a server name too long for the title bar is cut off inside it, and a short one is not', async () => {
	const long = await titleBar(
		300,
		'A server with a name much longer than the title bar has room for',
	)
	const bar = long.container.firstElementChild as HTMLElement
	await expect.poll(() => bar.style.getPropertyValue('--scroll-distance')).not.toBe('')
	expect(bar.getBoundingClientRect().width).toBeLessThanOrEqual(300)

	const short = await titleBar(300, 'Survival')
	await expect.element(short.getByText('Survival')).toBeVisible()
	expect(
		(short.container.firstElementChild as HTMLElement).style.getPropertyValue('--scroll-distance'),
	).toBe('')
})
