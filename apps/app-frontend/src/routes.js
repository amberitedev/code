import { createRouter, createWebHistory } from 'vue-router'

const Pages = {
	Index: () => import('@/pages/Index.vue'),
	Worlds: () => import('@/pages/Worlds.vue'),
	Servers: () => import('@/pages/Servers.vue'),
	Core: () => import('@/pages/core/Index.vue'),
	Browse: () => import('@/pages/Browse.vue'),
	Skins: () => import('@/pages/Skins.vue'),
}

const Hosting = {
	Index: () => import('@/pages/hosting/manage/Index.vue'),
	Overview: () => import('@/pages/hosting/manage/Overview.vue'),
	Content: () => import('@/pages/hosting/manage/Content.vue'),
	Files: () => import('@/pages/hosting/manage/Files.vue'),
	Backups: () => import('@/pages/hosting/manage/Backups.vue'),
	Access: () => import('@/pages/hosting/manage/Access.vue'),
	Browse: () => import('@/pages/hosting/manage/Browse.vue'),
}

const Instance = {
	Worlds: () => import('@/pages/instance/Worlds.vue'),
	Mods: () => import('@/pages/instance/Mods.vue'),
	Files: () => import('@/pages/instance/Files.vue'),
	Logs: () => import('@/pages/instance/Logs.vue'),
}

const Library = {
	Index: () => import('@/pages/library/Index.vue'),
	Overview: () => import('@/pages/library/Overview.vue'),
	Downloaded: () => import('@/pages/library/Downloaded.vue'),
	Modpacks: () => import('@/pages/library/Modpacks.vue'),
	Servers: () => import('@/pages/library/Servers.vue'),
	Custom: () => import('@/pages/library/Custom.vue'),
}

const Project = {
	Index: () => import('@/pages/project/Index.vue'),
	Description: () => import('@/pages/project/Description.vue'),
	Versions: () => import('@/pages/project/Versions.vue'),
	Version: () => import('@/pages/project/Version.vue'),
	Gallery: () => import('@/pages/project/Gallery.vue'),
}

const router = createRouter({
	history: createWebHistory(),
	routes: [
		{
			path: '/',
			name: 'Home',
			component: Pages.Index,
			meta: {
				breadcrumb: [{ name: 'Home' }],
			},
		},
		{
			path: '/worlds',
			name: 'Worlds',
			component: Pages.Worlds,
			meta: {
				breadcrumb: [{ name: 'Worlds' }],
			},
		},
		{
			path: '/core',
			name: 'Core',
			component: Pages.Core,
			meta: {
				breadcrumb: [{ name: 'Core' }],
			},
		},
		{
			path: '/hosting/manage/',
			name: 'Servers',
			component: Pages.Servers,
			meta: {
				breadcrumb: [{ name: 'Servers' }],
			},
		},
		{
			path: '/hosting/manage/:id',
			name: 'ServerManage',
			component: Hosting.Index,
			children: [
				{
					path: '',
					name: 'ServerManageOverview',
					component: Hosting.Overview,
					meta: {
						breadcrumb: [{ name: '?Server' }],
					},
				},
				{
					path: 'content',
					name: 'ServerManageContent',
					component: Hosting.Content,
					meta: {
						breadcrumb: [{ name: '?Server' }],
					},
				},
				{
					path: 'files',
					name: 'ServerManageFiles',
					component: Hosting.Files,
					meta: {
						breadcrumb: [{ name: '?Server' }],
					},
				},
				{
					path: 'backups',
					name: 'ServerManageBackups',
					component: Hosting.Backups,
					meta: {
						breadcrumb: [{ name: '?Server' }],
					},
				},
				{
					path: 'access',
					name: 'ServerManageAccess',
					component: Hosting.Access,
					meta: {
						breadcrumb: [{ name: '?Server' }],
					},
				},
				{
					path: 'browse',
					name: 'ServerManageBrowse',
					component: Hosting.Browse,
					meta: {
						breadcrumb: [{ name: '?Server' }, { name: 'Add content' }],
					},
				},
			],
		},
		{
			path: '/browse/:projectType',
			name: 'Discover content',
			component: Pages.Browse,
			meta: {
				useContext: true,
				breadcrumb: [{ name: '?BrowseTitle' }],
			},
		},
		{
			path: '/skins',
			name: 'Skin selector',
			component: Pages.Skins,
			meta: {
				breadcrumb: [{ name: 'Skin selector' }],
			},
		},
		{
			path: '/library',
			name: 'Library',
			component: Library.Index,
			meta: {
				breadcrumb: [{ name: 'Library' }],
			},
			children: [
				{
					path: '',
					name: 'Overview',
					component: Library.Overview,
				},
				{
					path: 'downloaded',
					name: 'Downloaded',
					component: Library.Downloaded,
				},
				{
					path: 'modpacks',
					name: 'Modpacks',
					component: Library.Modpacks,
				},
				{
					path: 'servers',
					name: 'LibraryServers',
					component: Library.Servers,
				},
				{
					path: 'custom',
					name: 'Custom',
					component: Library.Custom,
				},
			],
		},
		{
			path: '/:projectType(mod|plugin|datapack|resourcepack|shader|modpack)/:id/:rest(.*)*',
			redirect: (to) => {
				const rest = to.params.rest ? `/${[].concat(to.params.rest).join('/')}` : ''
				return `/project/${to.params.id}${rest}${to.hash}`
			},
		},
		{
			path: '/project/:id',
			name: 'Project',
			component: Project.Index,
			props: true,
			children: [
				{
					path: '',
					name: 'Description',
					component: Project.Description,
					meta: {
						useContext: true,
						breadcrumb: [{ name: '?Project' }],
					},
				},
				{
					path: 'versions',
					name: 'Versions',
					component: Project.Versions,
					meta: {
						useContext: true,
						breadcrumb: [{ name: '?Project', link: '/project/{id}/' }, { name: 'Versions' }],
					},
				},
				{
					path: 'version/:version',
					name: 'Version',
					component: Project.Version,
					props: true,
					meta: {
						useContext: true,
						breadcrumb: [
							{ name: '?Project', link: '/project/{id}/' },
							{ name: 'Versions', link: '/project/{id}/versions' },
							{ name: '?Version' },
						],
					},
				},
				{
					path: 'gallery',
					name: 'Gallery',
					component: Project.Gallery,
					meta: {
						useContext: true,
						breadcrumb: [{ name: '?Project', link: '/project/{id}/' }, { name: 'Gallery' }],
					},
				},
			],
		},
		{
			path: '/instance/:id',
			name: 'Instance',
			component: () => import('@/pages/instance/InstanceRouter.vue'),
			props: true,
			children: [
				{
					path: 'worlds',
					name: 'InstanceWorlds',
					component: Instance.Worlds,
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Worlds' }],
					},
				},
				{
					path: '',
					name: 'Mods',
					component: Instance.Mods,
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Content' }],
					},
				},
				{
					path: 'projects/:type',
					name: 'ModsFilter',
					component: Instance.Mods,
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Content' }],
					},
				},
				{
					path: 'content',
					name: 'ServerInstanceContent',
					component: () => import('@/pages/instance/ServerContent.vue'),
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Content' }],
					},
				},
				{
					path: 'browse',
					name: 'ServerInstanceBrowse',
					component: () => import('@/pages/instance/ServerBrowsePage.vue'),
					meta: {
						useRootContext: true,
						breadcrumb: [
							{ name: '?Instance', link: '/instance/{id}/content' },
							{ name: 'Add content' },
						],
					},
				},
				{
					path: 'files',
					name: 'Files',
					component: Instance.Files,
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Files' }],
					},
				},
				{
					path: 'backups',
					name: 'ServerInstanceBackups',
					component: () => import('@/pages/instance/ServerBackups.vue'),
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Backups' }],
					},
				},
				{
					path: 'access',
					name: 'ServerInstanceAccess',
					component: () => import('@/pages/instance/ServerAccess.vue'),
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Access' }],
					},
				},
				{
					path: 'logs',
					name: 'Logs',
					component: Instance.Logs,
					meta: {
						useRootContext: true,
						breadcrumb: [{ name: '?Instance', link: '/instance/{id}/' }, { name: 'Logs' }],
					},
				},
			],
		},
	],
	linkActiveClass: 'router-link-active',
	linkExactActiveClass: 'router-link-exact-active',
	scrollBehavior(to, from) {
		if (to.path === from.path) return
		document.querySelector('.app-viewport')?.scrollTo({ top: 0, left: 0, behavior: 'auto' })
	},
})

export default router
