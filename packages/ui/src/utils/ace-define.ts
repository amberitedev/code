import ace from 'ace-builds'

/** Registers an ace module. `ace.define` exists at runtime but is missing from the ace-builds types. */
export function aceDefine(
	name: string,
	dependencies: string[],
	factory: (...modules: never[]) => void,
): void {
	;(ace as typeof ace & { define: typeof aceDefine }).define(name, dependencies, factory)
}
