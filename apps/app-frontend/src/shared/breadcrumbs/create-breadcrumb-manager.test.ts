import { expect, test } from 'vite-plus/test'

import { createBreadcrumbManager } from './create-breadcrumb-manager'

const crumb = (slot: string, label: string) => ({ slot, id: `${slot}:${label}`, label })
const labels = (manager: ReturnType<typeof createBreadcrumbManager>) =>
	manager.entries.value.map((entry) => entry.label)

test('opening another page in a slot drops the pages that were under the old one', () => {
	const manager = createBreadcrumbManager()
	const servers = manager.reset(crumb('root', 'Servers'))
	const survival = manager.push(crumb('server', 'Survival'), { parent: servers })
	manager.push(crumb('tab', 'Files'), { parent: survival })
	expect(labels(manager)).toEqual(['Servers', 'Survival', 'Files'])

	manager.push(crumb('server', 'Creative'), { parent: servers })
	expect(labels(manager)).toEqual(['Servers', 'Creative'])
})
