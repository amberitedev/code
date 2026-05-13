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
		const coreId = body.core_id as string | undefined
		const ownerUserId = body.owner_user_id as string | undefined
		if (!coreId) throw new Error('Missing core_id')
		if (!ownerUserId) throw new Error('Missing owner_user_id')

		const supabaseUrl = Deno.env.get('SUPABASE_URL')!
		const serviceRoleKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!

		const supabaseAdmin = createClient(supabaseUrl, serviceRoleKey, {
			auth: { autoRefreshToken: false, persistSession: false },
		})

		// Look up existing registration
		const { data: existingReg } = await supabaseAdmin
			.from('core_registrations')
			.select('machine_user_id')
			.eq('id', coreId)
			.single()

		const email = `core-${coreId}@amberite.machine`
		const password = crypto.randomUUID()
		let userId: string

		if (existingReg?.machine_user_id) {
			userId = existingReg.machine_user_id
			// Update password
			const { error: updateError } = await supabaseAdmin.auth.admin.updateUserById(userId, {
				password,
				user_metadata: { core_id: coreId, account_type: 'machine' },
			})
			if (updateError) throw updateError
		} else {
			// Create new machine account
			const { data: newUser, error: createError } = await supabaseAdmin.auth.admin.createUser({
				email,
				password,
				email_confirm: true,
				user_metadata: { core_id: coreId, account_type: 'machine' },
			})
			if (createError) throw createError
			if (!newUser.user) throw new Error('User creation returned no user')
			userId = newUser.user.id

			// Upsert core_registrations
			const { error: regError } = await supabaseAdmin.from('core_registrations').upsert({
				id: coreId,
				owner_user_id: ownerUserId,
				machine_user_id: userId,
			})
			if (regError) throw regError
		}

		return new Response(
			JSON.stringify({ core_id: coreId, email, password, user_id: userId }),
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
