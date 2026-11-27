import { createContext } from '@modrinth/ui'
import { useTemplateRef } from 'vue'

import type { GameInstance } from '@/helpers/types'

import type InstanceActionModals from './instance-action-modals.vue'

interface InstanceActionsContext {
	confirmDelete: (instance: GameInstance) => void
	editIcon: (instance: GameInstance) => void
}

const [injectInstanceActions, provideInstanceActions] = createContext<InstanceActionsContext>(
	'InstanceActionModals',
	'instanceActions',
)

export function setupInstanceActionsProvider() {
	const modals = useTemplateRef<InstanceType<typeof InstanceActionModals>>('instanceActionModals')

	provideInstanceActions({
		confirmDelete: (instance) => modals.value?.confirmDelete(instance),
		editIcon: (instance) => void modals.value?.editIcon(instance),
	})
}

export { injectInstanceActions }
