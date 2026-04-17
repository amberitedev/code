/**
 * Mocked analytics helper - no-op
 * TODO: connect to backend API - replace with real analytics
 */

interface InstanceProperties {
	loader: string
	game_version: string
}

interface ProjectProperties extends InstanceProperties {
	id: string
	project_type: string
}

type AnalyticsEventMap = {
	Launched: { version: string; dev: boolean; onboarded: boolean }
	PageView: { path: string; fromPath: string; failed: unknown }
	InstanceCreate: { source: string }
	InstanceCreateStart: { source: string }
	InstanceStart: InstanceProperties & { source: string }
	InstanceStop: Partial<InstanceProperties> & { source?: string }
	InstanceDuplicate: InstanceProperties
	InstanceRepair: InstanceProperties
	InstanceSetIcon: Record<string, never>
	InstanceRemoveIcon: Record<string, never>
	InstanceUpdateAll: InstanceProperties & { count: number; selected: boolean }
	InstanceProjectUpdate: InstanceProperties & { id: string; name: string; project_type: string }
	InstanceProjectDisable: InstanceProperties & {
		id: string
		name: string
		project_type: string
		disabled: boolean
	}
	InstanceProjectRemove: InstanceProperties & { id: string; name: string; project_type: string }
	ProjectInstall: ProjectProperties & { version_id: string; title: string; source: string }
	ProjectInstallStart: { source: string }
	PackInstall: { id: string; version_id: string; title: string; source: string }
	PackInstallStart: Record<string, never>
	AccountLogIn: { source?: string }
	AccountLogOut: Record<string, never>
	JavaTest: { path: string; success: boolean }
	JavaManualSelect: { version: string }
	JavaAutoDetect: { path: string; version: string }
}

export type AnalyticsEvent = keyof AnalyticsEventMap

let initialized = false

export const initAnalytics = () => {
	// no-op in web mode
	initialized = true
}

export const debugAnalytics = () => {
	// no-op
}

export const optOutAnalytics = () => {
	// no-op
}

export const optInAnalytics = () => {
	// no-op
}

type OptionalArgs<T> = Record<string, never> extends T ? [properties?: T] : [properties: T]

export const trackEvent = <E extends AnalyticsEvent>(
	_eventName: E,
	..._args: OptionalArgs<AnalyticsEventMap[E]>
) => {
	// no-op in web mode
}
