import type { CreationFlowContextValue } from '@modrinth/ui'
import type { ComputedRef, Ref } from 'vue'

export type InstanceType = 'client' | 'server' | 'synced'
export type InstanceTypeClickBehavior = 'select' | 'continue'

export type InstanceCreationFlowContextValue = CreationFlowContextValue & {
	instanceType: Ref<InstanceType>
	instanceTypeClickBehavior: ComputedRef<InstanceTypeClickBehavior>
}
