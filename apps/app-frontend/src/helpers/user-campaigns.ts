import type { Labrinth } from '@modrinth/api-client'

type Pride26Campaign = Labrinth.Users.v3.Pride26CampaignDonation | null | undefined

export function hasPride26Badge(campaign: Pride26Campaign) {
	return campaign?.has_badge === true
}
