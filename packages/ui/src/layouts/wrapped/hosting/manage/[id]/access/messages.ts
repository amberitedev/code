import { defineMessages } from '#ui/composables/i18n'

export const accessMessages = defineMessages({
	activityLogTitle: {
		id: 'servers.access-page.activity-log-title',
		defaultMessage: 'Activity log',
	},
	addFilter: {
		id: 'servers.access-page.activity-log-filter.add',
		defaultMessage: 'Add filter',
	},
	clearFilters: {
		id: 'servers.access-page.activity-log-filter.clear',
		defaultMessage: 'Clear filters',
	},
	emptyFilterOptions: {
		id: 'servers.access-page.activity-log-filter.empty-options',
		defaultMessage: 'No options available.',
	},
	emptyFilterSearch: {
		id: 'servers.access-page.activity-log-filter.empty-search',
		defaultMessage: 'No options found.',
	},
	userFilter: {
		id: 'servers.access-page.activity-log-filter.users',
		defaultMessage: 'User',
	},
	supportActor: {
		id: 'servers.access-page.activity-log-filter.support-actor',
		defaultMessage: 'Support',
	},
	actionTypeFilter: {
		id: 'servers.access-page.activity-log-filter.action-types',
		defaultMessage: 'Actions',
	},
	actionTypeFilterSearch: {
		id: 'servers.access-page.activity-log-filter.action-types-search',
		defaultMessage: 'Search actions...',
	},
	loadFailedTitle: {
		id: 'servers.access-page.notification.activity-load-failed.title',
		defaultMessage: 'Activity could not be loaded',
	},
	loadFailedText: {
		id: 'servers.access-page.notification.load-failed.text',
		defaultMessage: 'Refresh the page to try again.',
	},
})

export const actionLogActionMessages = defineMessages({
	server_created: {
		id: 'servers.access-page.activity-log-filter.action.server-created',
		defaultMessage: 'Created server',
	},
	server_started: {
		id: 'servers.access-page.activity-log-filter.action.server-started',
		defaultMessage: 'Started server',
	},
	server_stopped: {
		id: 'servers.access-page.activity-log-filter.action.server-stopped',
		defaultMessage: 'Stopped server',
	},
	server_restarted: {
		id: 'servers.access-page.activity-log-filter.action.server-restarted',
		defaultMessage: 'Restarted server',
	},
	server_killed: {
		id: 'servers.access-page.activity-log-filter.action.server-killed',
		defaultMessage: 'Killed server',
	},
	console_command_executed: {
		id: 'servers.access-page.activity-log-filter.action.console-command-executed',
		defaultMessage: 'Ran console command',
	},
	content_changed: {
		id: 'servers.access-page.activity-log-filter.action.content-changed',
		defaultMessage: 'Changed content',
	},
	settings_changed: {
		id: 'servers.access-page.activity-log-filter.action.settings-changed',
		defaultMessage: 'Changed settings',
	},
	backup_created: {
		id: 'servers.access-page.activity-log-filter.action.backup-created',
		defaultMessage: 'Created backup',
	},
	backup_renamed: {
		id: 'servers.access-page.activity-log-filter.action.backup-renamed',
		defaultMessage: 'Renamed backup',
	},
	backup_restored: {
		id: 'servers.access-page.activity-log-filter.action.backup-restored',
		defaultMessage: 'Restored backup',
	},
	backup_deleted: {
		id: 'servers.access-page.activity-log-filter.action.backup-deleted',
		defaultMessage: 'Deleted backup',
	},
	backup_downloaded: {
		id: 'servers.access-page.activity-log-filter.action.backup-downloaded',
		defaultMessage: 'Downloaded backup',
	},
	world_downloaded: {
		id: 'servers.access-page.activity-log-filter.action.world-downloaded',
		defaultMessage: 'Downloaded world',
	},
})
