import { FixtureError } from '../src/context.ts'

/**
 * Mods by name, each pinned to one Modrinth version per loader and game version, so a fixture
 * installs the same file every time. The ids are the last part of a version's page address.
 */
const MODS: Readonly<Record<string, Readonly<Record<string, string>>>> = {
	'ferrite-core': { 'fabric 1.21.1': 'zQSo4nxk' },
}

/** The pinned version of `mod` for an instance, or the id itself when a fixture passes one. */
export function pinnedVersion(mod: string, loader: string, gameVersion: string): string {
	const versions = MODS[mod]
	// A name the list lacks is taken as a version id, which Modrinth writes as eight characters.
	if (!versions && /^[A-Za-z0-9]{8}$/.test(mod)) return mod
	const version = versions?.[`${loader} ${gameVersion}`]
	if (!version) {
		throw new FixtureError(
			`No pinned version of ${mod} for ${loader} ${gameVersion}. Add one to packages/fixtures/kit/mods.ts, or pass a Modrinth version id.`,
		)
	}
	return version
}
