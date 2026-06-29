export type ReadyTransitionState =
	| 'idle'
	| 'pending-hidden'
	| 'ghost-visible'
	| 'resolved'
	| 'timeout'
	| 'error'

export interface ReadyTransitionSlotProps {
	state: ReadyTransitionState
	timedOut: boolean
	error: unknown
}
