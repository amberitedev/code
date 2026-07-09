import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { MutationCtx, QueryCtx } from './_generated/server'
import {
	banByGroupUser,
	membershipByGroupUser,
	requireFriendGroupRole,
	requireSingleGroupMembership,
	requireSingleOwnedCore,
	resolveActor,
} from './_socialRules'

const role = v.union(v.literal('owner'), v.literal('admin'), v.literal('member'))
const devActAs = { __actAs: v.optional(v.string()) }

export const createFriendGroupInvite = mutation({
	args: {
		friendGroupId: v.string(),
		inviteeUserId: v.optional(v.string()),
		role: v.optional(role),
		ttlMs: v.optional(v.number()),
		...devActAs,
	},
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin'])
		if (args.inviteeUserId) {
			const ban = await banByGroupUser(ctx, args.friendGroupId, args.inviteeUserId)
			if (ban) throw new Error('user is banned from this friend group')
		}
		const now = Date.now()
		const code = args.inviteeUserId ? undefined : await createInviteCode(ctx)
		const inviteId = await ctx.db.insert('friendGroupInvites', {
			friendGroupId: args.friendGroupId,
			inviterUserId: userId,
			inviteeUserId: args.inviteeUserId,
			code,
			role: args.role ?? 'member',
			status: 'pending',
			createdAt: now,
			expiresAt: now + (args.ttlMs ?? 7 * 24 * 60 * 60 * 1000),
		})
		return { inviteId, code }
	},
})

export const listMyGroupInvites = query({
	args: { ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const invites = await invitesByUser(ctx, userId)
		return await Promise.all(
			invites.map(async (invite) => ({
				invite,
				group: publicFriendGroup(await ctx.db.get(invite.friendGroupId as any)),
			})),
		)
	},
})

export const getInviteByCode = query({
	args: { code: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		await resolveActor(ctx, args.__actAs)
		const invite = await inviteByCode(ctx, args.code)
		if (!invite || invite.status !== 'pending' || invite.expiresAt <= Date.now()) return null
		const group = await ctx.db.get(invite.friendGroupId as any)
		if (!group) return null
		return {
			invite: { inviteId: invite._id, role: invite.role, expiresAt: invite.expiresAt },
			group: { id: group._id, name: group.name },
		}
	},
})

export const acceptFriendGroupInvite = mutation({
	args: {
		inviteId: v.optional(v.id('friendGroupInvites')),
		code: v.optional(v.string()),
		...devActAs,
	},
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const invite = await resolveInvite(ctx, args)
		if (!invite || invite.status !== 'pending' || invite.expiresAt <= Date.now())
			throw new Error('invite not found')
		if (invite.inviteeUserId && invite.inviteeUserId !== userId)
			throw new Error('invite belongs to another user')
		const ban = await banByGroupUser(ctx, invite.friendGroupId, userId)
		if (ban) throw new Error('you are banned from this friend group')
		await requireSingleGroupMembership(ctx, userId, invite.friendGroupId)
		const existing = await membershipByGroupUser(ctx, invite.friendGroupId, userId)
		const now = Date.now()
		if (!existing) {
			await requireSingleOwnedCore(ctx, userId)
			await ctx.db.insert('friendGroupMembers', {
				friendGroupId: invite.friendGroupId,
				userId,
				role: invite.role,
				permissionPreset: invite.role,
				createdAt: now,
				updatedAt: now,
			})
		}
		await ctx.db.patch(invite._id, { status: 'accepted', respondedAt: now })
		return { friendGroupId: invite.friendGroupId }
	},
})

export const declineFriendGroupInvite = mutation({
	args: { inviteId: v.id('friendGroupInvites'), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const invite = await ctx.db.get(args.inviteId)
		if (!invite || invite.inviteeUserId !== userId) throw new Error('invite not found')
		await ctx.db.patch(args.inviteId, { status: 'declined', respondedAt: Date.now() })
		return null
	},
})

export const revokeFriendGroupInvite = mutation({
	args: { inviteId: v.id('friendGroupInvites'), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const invite = await ctx.db.get(args.inviteId)
		if (!invite) return null
		await requireFriendGroupRole(ctx, userId, invite.friendGroupId, ['owner', 'admin'])
		await ctx.db.patch(args.inviteId, { status: 'revoked', respondedAt: Date.now() })
		return null
	},
})

async function resolveInvite(ctx: QueryCtx, args: { inviteId?: string; code?: string }) {
	if (args.inviteId) return await ctx.db.get(args.inviteId as any)
	if (args.code) return await inviteByCode(ctx, args.code)
	return null
}

async function createInviteCode(ctx: MutationCtx): Promise<string> {
	for (let i = 0; i < 10; i++) {
		const code = secureInviteCode()
		const existing = await inviteByCode(ctx, code)
		if (!existing) return code
	}
	throw new Error('could not allocate invite code')
}

function secureInviteCode(): string {
	const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'
	const bytes = crypto.getRandomValues(new Uint8Array(8))
	return Array.from(bytes, (byte) => alphabet[byte % alphabet.length]).join('')
}

async function invitesByUser(ctx: QueryCtx, userId: string) {
	const now = Date.now()
	const invites = await ctx.db
		.query('friendGroupInvites')
		.withIndex('by_invitee_status', (q) => q.eq('inviteeUserId', userId).eq('status', 'pending'))
		.collect()
	return invites.filter((invite) => invite.expiresAt > now)
}
function inviteByCode(ctx: QueryCtx | MutationCtx, code: string) {
	return ctx.db
		.query('friendGroupInvites')
		.withIndex('by_code', (q) => q.eq('code', code.trim().toUpperCase()))
		.unique()
}

function publicFriendGroup(group: any) {
	return group ? { ...group, id: group._id } : null
}
