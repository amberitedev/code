import type { Labrinth } from '@modrinth/api-client'

/**
 * Dev-mode auth bypass.
 * Seeds a mock user into useState('auth') so all hosting pages are browsable
 * without a real Modrinth/Labrinth backend.
 *
 * Runs before all named middleware (including 'auth') because it is global (.global.ts).
 * No-ops in production.
 */

// Same mock user as hosting-mock.client.ts for consistency
const MOCK_USER: Labrinth.Users.v2.User = {
  id: 'mock-user-id',
  username: 'devuser',
  name: 'Dev User',
  email: 'dev@example.com',
  avatar_url: null,
  bio: null,
  created: '2024-01-01T00:00:00Z',
  role: 'developer',
  badges: 0,
  auth_providers: null,
  email_verified: true,
  has_password: true,
  has_totp: false,
  payout_data: null,
  stripe_customer_id: 'cus_mock_12345',
}

export default defineNuxtRouteMiddleware(() => {
  if (!import.meta.dev) return

  const auth = useState('auth', () => ({
    user: null as Labrinth.Users.v2.User | null,
    token: '',
    headers: {} as Record<string, string>,
  }))

  if (auth.value.user) return // already seeded on a previous navigation

  const mockToken = 'mra_devmocktoken00000000000000000'

  auth.value = {
    user: MOCK_USER,
    token: mockToken,
    headers: { Authorization: `Bearer ${mockToken}` },
  }

  console.log('[dev-mock] Authenticated as:', MOCK_USER.username)
})
