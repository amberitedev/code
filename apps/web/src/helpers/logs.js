/**
 * Mocked logs helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

export async function get_logs(profilePath, clearContents) {
	return []
}

export async function get_logs_by_filename(profilePath, logType, filename) {
	return {
		filename: '',
		stdout: '',
		stderr: '',
	}
}

export async function get_output_by_filename(profilePath, logType, filename) {
	return ''
}

export async function delete_logs_by_filename(profilePath, logType, filename) {
	// no-op
}

export async function delete_logs(profilePath) {
	// no-op
}

export async function get_latest_log_cursor(profilePath, cursor) {
	return {
		cursor: 0,
		output: '',
		new_file: true,
	}
}
