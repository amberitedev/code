import type {
	ServerAccessInviteSuggestion,
	ServerAccessRole,
	ServerAccessRoleOption,
} from '@modrinth/ui'
import type { ComputedRef, InjectionKey, Ref } from 'vue'
import { inject, provide } from 'vue'

import type { CoreAccessMember } from './core-access-types'

export type CoreOnboardingFlow = 'create' | 'connect'

export interface CoreOnboardingContext {
	flow: Ref<CoreOnboardingFlow>
	connectCode: Ref<string>
	connectValidated: Ref<boolean>
	inviteSearch: Ref<string>
	inviteAsFriend: Ref<boolean>
	error: Ref<string>
	working: Ref<boolean>
	canManage: ComputedRef<boolean>
	members: ComputedRef<CoreAccessMember[]>
	roles: ServerAccessRoleOption[]
	inviteSuggestions: ComputedRef<ServerAccessInviteSuggestion[]>
	selectInviteSuggestion: (user: ServerAccessInviteSuggestion) => void
	createInvite: () => void
	quickInvite: (member: CoreAccessMember) => void
	updateRole: (member: CoreAccessMember, role: ServerAccessRole) => Promise<void>
	removeMember: (member: CoreAccessMember) => Promise<void>
}

const coreOnboardingKey: InjectionKey<CoreOnboardingContext> = Symbol('CoreOnboarding')

export function provideCoreOnboardingContext(ctx: CoreOnboardingContext) {
	provide(coreOnboardingKey, ctx)
}

export function injectCoreOnboardingContext() {
	const ctx = inject(coreOnboardingKey)
	if (!ctx) throw new Error('Core onboarding context is not provided')
	return ctx
}
