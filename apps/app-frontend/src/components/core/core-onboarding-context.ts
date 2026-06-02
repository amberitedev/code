import type { AmberiteUser } from '@amberite/amberite-api'
import type { InjectionKey, Ref } from 'vue'
import { inject, provide } from 'vue'

export type OnboardingMode = 'setup' | 'connect'

export interface InvitedUser {
	userId: string
	username: string
	image?: string
}

export interface OnboardingContext {
	mode: OnboardingMode
	code: Ref<string>
	step1Loading: Ref<boolean>
	step1Error: Ref<string | null>
	alreadyPaired: Ref<boolean>
	coreId: Ref<string | null>
	groupName: Ref<string>
	description: Ref<string>
	bannerUrl: Ref<string>
	subdomain: Ref<string>
	runOnStartup: Ref<boolean>
	runInBackground: Ref<boolean>
	inviteQuery: Ref<string>
	inviteSearchResults: Ref<AmberiteUser[]>
	inviteSearchLoading: Ref<boolean>
	invitedUsers: Ref<InvitedUser[]>
	generatedInviteCode: Ref<string | null>
	runMode: Ref<'manual' | 'app_open' | 'startup'>
	pair: () => Promise<void>
	join: () => Promise<void>
	searchUsers: () => Promise<void>
	inviteUser: (userId: string, role: string) => Promise<void>
	generateInviteLink: (role: string) => Promise<void>
	saveAdvanced: () => Promise<void>
	finish: () => void
}

export const OnboardingKey: InjectionKey<OnboardingContext> = Symbol('CoreOnboarding')

export function provideOnboarding(ctx: OnboardingContext) {
	provide(OnboardingKey, ctx)
}

export function useOnboarding(): OnboardingContext {
	const ctx = inject(OnboardingKey)
	if (!ctx) throw new Error('useOnboarding must be used inside CoreOnboardingModal')
	return ctx
}
