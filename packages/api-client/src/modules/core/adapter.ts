/** Platform capabilities required to communicate with a Copal Core. */
export interface CoreClientAdapter {
	fetchFn: typeof fetch
	getCoreUrl(): Promise<string | null>
	getConnectedCoreId?(): Promise<string | null>
	getCurrentJwt(): Promise<string | null>
	getLocalSetupSecret?(): Promise<string | null>
}
