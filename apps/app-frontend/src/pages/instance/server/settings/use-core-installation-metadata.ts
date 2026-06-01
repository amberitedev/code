import type { GameVersionTag } from '@modrinth/utils'
import { onMounted, shallowRef } from 'vue'

import { get_loader_versions } from '@/helpers/metadata'
import { get_game_versions } from '@/helpers/tags'
import type { Manifest } from '@/helpers/types'

/**
 * Loads launcher metadata (loader manifests + game versions) and exposes the
 * synchronous resolver functions the InstallationSettingsLayout expects.
 * Paper/Vanilla have no launcher manifest, so their loader versions resolve to
 * an empty list and Core installs the latest build on change.
 */
export function useCoreInstallationMetadata(handleError: (error: unknown) => void) {
	const fabricVersions = shallowRef<Manifest | null>(null)
	const forgeVersions = shallowRef<Manifest | null>(null)
	const quiltVersions = shallowRef<Manifest | null>(null)
	const neoforgeVersions = shallowRef<Manifest | null>(null)
	const allGameVersions = shallowRef<GameVersionTag[]>([])

	async function loadMetadata() {
		const [fabric, forge, quilt, neo, games] = await Promise.all([
			get_loader_versions('fabric').catch(handleError),
			get_loader_versions('forge').catch(handleError),
			get_loader_versions('quilt').catch(handleError),
			get_loader_versions('neo').catch(handleError),
			get_game_versions().catch(handleError),
		])
		fabricVersions.value = (fabric as Manifest) ?? null
		forgeVersions.value = (forge as Manifest) ?? null
		quiltVersions.value = (quilt as Manifest) ?? null
		neoforgeVersions.value = (neo as Manifest) ?? null
		allGameVersions.value = (games as GameVersionTag[]) ?? []
	}
	onMounted(loadMetadata)

	function getManifest(loader: string) {
		const map: Record<string, typeof fabricVersions> = {
			fabric: fabricVersions,
			forge: forgeVersions,
			quilt: quiltVersions,
			neoforge: neoforgeVersions,
		}
		return map[loader]
	}

	function resolveGameVersions(loader: string, showSnapshots: boolean) {
		const versions = allGameVersions.value
		const filtered = versions.filter((item) => {
			if (loader === 'vanilla' || loader === 'paper') return true
			const manifest = getManifest(loader)
			return !!manifest?.value?.gameVersions?.some((x) => item.version === x.id)
		})
		return (showSnapshots ? filtered : filtered.filter((x) => x.version_type === 'release')).map(
			(x) => ({ value: x.version, label: x.version }),
		)
	}

	function resolveLoaderVersions(loader: string, gameVersion: string) {
		if (loader === 'vanilla' || loader === 'paper' || !gameVersion) return []
		const manifest = getManifest(loader)
		if (!manifest?.value) return []
		if (loader === 'fabric' || loader === 'quilt') {
			return manifest.value.gameVersions[0]?.loaders ?? []
		}
		return manifest.value.gameVersions?.find((item) => item.id === gameVersion)?.loaders ?? []
	}

	function resolveHasSnapshots(loader: string) {
		const versions = allGameVersions.value
		if (loader === 'vanilla' || loader === 'paper')
			return versions.some((x) => x.version_type !== 'release')
		const manifest = getManifest(loader)
		const supported = versions.filter(
			(item) => !!manifest?.value?.gameVersions?.some((x) => item.version === x.id),
		)
		return supported.some((x) => x.version_type !== 'release')
	}

	return { resolveGameVersions, resolveLoaderVersions, resolveHasSnapshots }
}
