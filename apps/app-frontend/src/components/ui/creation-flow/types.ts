import type { CreationFlowContextValue } from '@modrinth/ui'
import type { Ref } from 'vue'

export type InstanceType = 'client' | 'server' | 'synced'

export type InstanceCreationFlowContextValue = CreationFlowContextValue & {
	instanceType: Ref<InstanceType>
}
