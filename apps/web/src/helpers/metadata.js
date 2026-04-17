/**
 * Mocked metadata helper - returns empty/fake data
 * TODO: connect to backend API - fetch game versions and loader versions
 */

export async function get_game_versions() {
	return {
		gameVersions: [],
	}
}

export async function get_loader_versions(loader) {
	return {
		id: '',
		url: '',
		stable: false,
	}
}
