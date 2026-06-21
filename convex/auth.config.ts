const convexSiteUrl = process.env.CONVEX_SITE_URL

if (!convexSiteUrl?.trim()) {
	throw new Error('Missing required environment variable: CONVEX_SITE_URL')
}

export default {
	providers: [
		{
			domain: convexSiteUrl,
			applicationID: "convex",
		},
	],
};
