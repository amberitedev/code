import { createClient } from 'https://esm.sh/@supabase/supabase-js@2'

const corsHeaders = {
	'Access-Control-Allow-Origin': '*',
	'Access-Control-Allow-Headers': 'authorization, x-client-info, apikey, content-type',
	'Access-Control-Allow-Methods': 'POST, OPTIONS',
}

Deno.serve(async (req) => {
	if (req.method === 'OPTIONS') {
		return new Response('ok', { headers: corsHeaders })
	}

	try {
		const body = await req.json()
		const accessToken = body.access_token as string | undefined
		const idToken = body.id_token as string | undefined
		if (!accessToken) throw new Error('Missing access_token')

		// 1. Xbox auth
		const xboxRes = await fetch('https://user.auth.xboxlive.com/user/authenticate', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
			body: JSON.stringify({
				Properties: { AuthMethod: 'RPS', SiteName: 'user.auth.xboxlive.com', RpsTicket: `d=${accessToken}` },
				RelyingParty: 'http://auth.xboxlive.com',
				TokenType: 'JWT',
			}),
		})
		if (!xboxRes.ok) throw new Error(`Xbox auth failed: ${xboxRes.status}`)
		const xboxData = await xboxRes.json()
		const xboxToken = xboxData.Token as string
		const uhs = xboxData.DisplayClaims?.xui?.[0]?.uhs as string
		if (!xboxToken || !uhs) throw new Error('Invalid Xbox response')

		// 2. XSTS auth
		const xstsRes = await fetch('https://xsts.auth.xboxlive.com/xsts/authorize', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
			body: JSON.stringify({
				Properties: { SandboxId: 'RETAIL', UserTokens: [xboxToken] },
				RelyingParty: 'rp://api.minecraftservices.com/',
				TokenType: 'JWT',
			}),
		})
		if (!xstsRes.ok) throw new Error(`XSTS auth failed: ${xstsRes.status}`)
		const xstsData = await xstsRes.json()
		const xstsToken = xstsData.Token as string
		if (!xstsToken) throw new Error('Invalid XSTS response')

		// 3. Minecraft auth
		const mcAuthRes = await fetch('https://api.minecraftservices.com/authentication/login_with_xbox', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ identityToken: `XBL3.0 x=${uhs};${xstsToken}` }),
		})
		if (!mcAuthRes.ok) throw new Error(`Minecraft auth failed: ${mcAuthRes.status}`)
		const mcAuthData = await mcAuthRes.json()
		const mcAccessToken = mcAuthData.access_token as string
		if (!mcAccessToken) throw new Error('Invalid Minecraft auth response')

		// 4. Minecraft profile
		const profileRes = await fetch('https://api.minecraftservices.com/minecraft/profile', {
			headers: { Authorization: `Bearer ${mcAccessToken}` },
		})
		if (!profileRes.ok) throw new Error(`Minecraft profile failed: ${profileRes.status}`)
		const profile = await profileRes.json()
		const minecraftUuid = profile.id as string
		const minecraftName = profile.name as string
		if (!minecraftUuid || !minecraftName) throw new Error('Invalid Minecraft profile')

		// 5. Supabase setup
		const supabaseUrl = Deno.env.get('SUPABASE_URL')!
		const serviceRoleKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!
		const anonKey = Deno.env.get('SUPABASE_ANON_KEY')!

		const supabaseAdmin = createClient(supabaseUrl, serviceRoleKey, {
			auth: { autoRefreshToken: false, persistSession: false },
		})
		const supabaseClient = createClient(supabaseUrl, anonKey, {
			auth: { autoRefreshToken: false, persistSession: false },
		})

		// 6. Look up existing identity
		const { data: existingIdentity } = await supabaseAdmin
			.from('minecraft_identities')
			.select('supabase_user_id')
			.eq('minecraft_uuid', minecraftUuid)
			.single()

		let userId: string
		const email = `${minecraftUuid}@amberite.minecraft`
		const password = crypto.randomUUID()

		if (existingIdentity?.supabase_user_id) {
			userId = existingIdentity.supabase_user_id
			// Update password so we can sign in
			const { error: updateError } = await supabaseAdmin.auth.admin.updateUserById(userId, {
				password,
				user_metadata: { minecraft_uuid: minecraftUuid, minecraft_name: minecraftName },
			})
			if (updateError) throw updateError
		} else {
			// Create new user
			const { data: newUser, error: createError } = await supabaseAdmin.auth.admin.createUser({
				email,
				password,
				email_confirm: true,
				user_metadata: { minecraft_uuid: minecraftUuid, minecraft_name: minecraftName },
			})
			if (createError) throw createError
			if (!newUser.user) throw new Error('User creation returned no user')
			userId = newUser.user.id

			// Insert identity mapping
			const { error: identityError } = await supabaseAdmin.from('minecraft_identities').insert({
				minecraft_uuid: minecraftUuid,
				minecraft_name: minecraftName,
				supabase_user_id: userId,
			})
			if (identityError) throw identityError
		}

		// 7. Sign in to get a session
		const { data: signInData, error: signInError } = await supabaseClient.auth.signInWithPassword({
			email,
			password,
		})
		if (signInError || !signInData.session) throw new Error(signInError?.message ?? 'Sign in failed')

		return new Response(
			JSON.stringify({
				access_token: signInData.session.access_token,
				refresh_token: signInData.session.refresh_token,
				expires_in: signInData.session.expires_in,
				user_id: userId,
				display_name: minecraftName,
			}),
			{ headers: { ...corsHeaders, 'Content-Type': 'application/json' } },
		)
	} catch (err) {
		const message = err instanceof Error ? err.message : String(err)
		return new Response(
			JSON.stringify({ error: message }),
			{ status: 400, headers: { ...corsHeaders, 'Content-Type': 'application/json' } },
		)
	}
})
