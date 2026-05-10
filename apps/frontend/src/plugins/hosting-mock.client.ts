/**
 * AMBERITE PATCH — comprehensive dev-only fetch interceptor for ALL hosting & billing APIs.
 *
 * This plugin intercepts:
 * 1. Archon API calls (hosting servers, worlds, backups, etc.)
 * 2. Labrinth billing API calls (payments, subscriptions, customer data)
 * 3. Authentication checks
 *
 * Returns realistic mock data so the ENTIRE hosting UI works without any backend.
 * Never hits real servers. Always returns "authenticated" with a fake user.
 *
 * Production guard ensures this is a no-op in production builds.
 */

import { defineNuxtPlugin, useRuntimeConfig } from '#app'

// ── Mock User ─────────────────────────────────────────────────────────────────

const MOCK_USER = {
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

// ── Mock IDs ──────────────────────────────────────────────────────────────────

const MOCK_SERVER_ID = '00000000-0000-0000-0000-000000000001'
const MOCK_WORLD_ID = '00000000-0000-0000-0000-000000000002'
const MOCK_OWNER_ID = 'mock-user-id'
const MOCK_SUBSCRIPTION_ID = 'sub_mock_12345'

// ── Mock Server Data (Archon v0) ──────────────────────────────────────────────

const MOCK_SERVER_V0 = {
  server_id: MOCK_SERVER_ID,
  name: 'Mock Server',
  owner_id: MOCK_OWNER_ID,
  net: { ip: '127.0.0.1', port: 25565, domain: 'mock.modrinth.gg' },
  game: 'Minecraft',
  backup_quota: 5,
  used_backup_quota: 0,
  status: 'available',
  suspension_reason: null,
  loader: 'Fabric',
  loader_version: '0.16.14',
  mc_version: '1.21.1',
  upstream: null,
  sftp_username: 'mock.sftp',
  sftp_password: 'mock-sftp-password',
  sftp_host: 'sftp.mock.modrinth.gg',
  datacenter: 'us-east-1',
  notices: [],
  node: null,
  flows: { intro: false },
  is_medal: false,
}

// ── Mock World ────────────────────────────────────────────────────────────────

const MOCK_WORLD = {
  id: MOCK_WORLD_ID,
  name: 'World 1',
  created_at: '2025-01-01T00:00:00.000Z',
  is_active: true,
  backups: [],
  content: {
    modloader: 'fabric',
    modloader_version: '0.16.14',
    game_version: '1.21.1',
    java_version: 21,
    invocation: 'java -Xms128M -Xmx6144M -jar server.jar --nogui',
    original_invocation: 'java -Xms128M -Xmx6144M -jar server.jar --nogui',
  },
  readiness: { data_synchronized_fetched: true },
}

// ── Mock Server Data (Archon v1) ──────────────────────────────────────────────

const MOCK_SERVER_V1 = {
  id: MOCK_SERVER_ID,
  name: 'Mock Server',
  subdomain: 'mock-server',
  specs: { cpu: 200, memory_mb: 6144, storage_mb: 51200, swap_mb: 0 },
  sftp_username: 'mock.sftp',
  sftp_password: 'mock-sftp-password',
  tags: [],
  location: {
    status: 'assigned',
    location_metadata: {
      region: 'us-east',
      region_should_be_user_displayed: true,
      hostname: 'mock.modrinth.gg',
      is_decommissioned_node: false,
    },
  },
  worlds: [MOCK_WORLD],
}

// ── Mock Region ───────────────────────────────────────────────────────────────

const MOCK_REGION = {
  shortcode: 'us-east',
  country_code: 'US',
  display_name: 'US East',
  lat: 40.7128,
  lon: -74.006,
  zone: 'modrinth.gg',
}

// ── Mock Configs ──────────────────────────────────────────────────────────────

const MOCK_STARTUP_CONFIG = {
  invocation: 'java -Xms128M -Xmx6144M -jar server.jar --nogui',
  original_invocation: 'java -Xms128M -Xmx6144M -jar server.jar --nogui',
  jdk_version: 'lts21',
  jdk_build: 'temurin',
}

const MOCK_RUNTIME_OPTIONS = {
  java_version: 21,
  jre_vendor: 'temurin',
  original_invocation: 'java -Xms128M -Xmx6144M -jar server.jar --nogui',
  startup_command: null,
}

const MOCK_PROPERTIES = {
  known: {
    difficulty: 'normal',
    gamemode: 'survival',
    max_players: '20',
    motd: 'A Mock Minecraft Server',
    view_distance: '10',
    white_list: 'false',
  },
  custom: {},
}

const MOCK_ADDONS = {
  modloader: 'fabric',
  modloader_version: '0.16.14',
  game_version: '1.21.1',
  modpack: null,
  addons: [],
}

const MOCK_BACKUPS_QUEUE = {
  active_operations: [],
  backups: [],
}

// ── Mock Billing Data ─────────────────────────────────────────────────────────

const MOCK_CUSTOMER = {
  id: 'cus_mock_12345',
  user_id: MOCK_USER.id,
  balance: 0,
  currency: 'usd',
  default_payment_method: null,
}

const MOCK_PAYMENT_METHODS = []

const MOCK_SUBSCRIPTION = {
  id: MOCK_SUBSCRIPTION_ID,
  user_id: MOCK_USER.id,
  product_id: 'prod_mock_server',
  status: 'active',
  current_period_start: '2025-01-01T00:00:00Z',
  current_period_end: '2025-12-31T23:59:59Z',
  cancel_at_period_end: false,
  canceled_at: null,
  metadata: {},
}

const MOCK_PRODUCTS = [
  {
    id: 'prod_mock_server',
    name: 'Mock Server Plan',
    description: 'A mock server for development',
    prices: {
      intervals: {
        monthly: 0,
        quarterly: 0,
        yearly: 0,
      },
    },
    metadata: {},
  },
]

const MOCK_PAYMENTS = []

// ── Helper Functions ──────────────────────────────────────────────────────────

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

function empty(): Response {
  return new Response(null, { status: 204 })
}

function notFound(endpoint: string): Response {
  console.warn(`[hosting-mock] 404: ${endpoint}`)
  return json({ error: 'Not found', message: 'Mock endpoint not implemented' }, 404)
}

// ── Archon Router ─────────────────────────────────────────────────────────────

function routeArchon(method: string, pathname: string): Response | null {
  // ── Archon v0 (/modrinth/v0/...) ──────────────────────────────────────────

  // GET /modrinth/v0/servers - List all servers
  if (method === 'GET' && /^\/modrinth\/v0\/servers$/.test(pathname)) {
    return json({
      servers: [MOCK_SERVER_V0],
      pagination: { current_page: 1, page_size: 100, total_pages: 1, total_items: 1 },
    })
  }

  // POST /modrinth/v0/stock - Check server stock availability
  if (method === 'POST' && /^\/modrinth\/v0\/stock/.test(pathname)) {
    return json({ available: 999 })
  }

  // GET /modrinth/v0/subdomains/:sub/isavailable
  if (method === 'GET' && /^\/modrinth\/v0\/subdomains\/[^/]+\/isavailable$/.test(pathname)) {
    return json({ available: true })
  }

  // GET /modrinth/v0/servers/:id - Get server details
  if (method === 'GET' && /^\/modrinth\/v0\/servers\/[^/]+$/.test(pathname)) {
    return json(MOCK_SERVER_V0)
  }

  // GET /modrinth/v0/servers/:id/ws - Get WebSocket connection info
  if (method === 'GET' && /^\/modrinth\/v0\/servers\/[^/]+\/ws$/.test(pathname)) {
    return json({ url: 'wss://localhost:9999/mock-ws', token: 'mock-ws-token' })
  }

  // GET /modrinth/v0/servers/:id/fs - Get file system access
  if (method === 'GET' && /^\/modrinth\/v0\/servers\/[^/]+\/fs$/.test(pathname)) {
    return json({ url: 'localhost:9999', token: 'mock-fs-token' })
  }

  // GET /modrinth/v0/servers/:id/allocations - List port allocations
  if (method === 'GET' && /^\/modrinth\/v0\/servers\/[^/]+\/allocations$/.test(pathname)) {
    return json([])
  }

  // POST /modrinth/v0/servers/:id/allocations - Create allocation
  if (method === 'POST' && /^\/modrinth\/v0\/servers\/[^/]+\/allocations$/.test(pathname)) {
    return json({ port: 25566, name: 'mock' })
  }

  // PUT/DELETE /modrinth/v0/servers/:id/allocations/:port
  if (
    (method === 'PUT' || method === 'DELETE') &&
    /^\/modrinth\/v0\/servers\/[^/]+\/allocations\/\d+$/.test(pathname)
  ) {
    return empty()
  }

  // GET /modrinth/v0/servers/:id/startup - Get startup config
  if (method === 'GET' && /^\/modrinth\/v0\/servers\/[^/]+\/startup$/.test(pathname)) {
    return json(MOCK_STARTUP_CONFIG)
  }

  // POST /modrinth/v0/servers/:id/startup - Update startup config
  if (method === 'POST' && /^\/modrinth\/v0\/servers\/[^/]+\/startup$/.test(pathname)) {
    return empty()
  }

  // POST /modrinth/v0/servers/:id/power - Power control (start/stop/restart)
  if (method === 'POST' && /^\/modrinth\/v0\/servers\/[^/]+\/power$/.test(pathname)) {
    return empty()
  }

  // POST /modrinth/v0/servers/:id/reinstall - Reinstall server
  if (method === 'POST' && /^\/modrinth\/v0\/servers\/[^/]+\/reinstall$/.test(pathname)) {
    return empty()
  }

  // GET /modrinth/v0/servers/:id/reinstallFromMrpack
  if (method === 'GET' && /^\/modrinth\/v0\/servers\/[^/]+\/reinstallFromMrpack$/.test(pathname)) {
    return json({ url: 'localhost:9999', token: 'mock-mrpack-token' })
  }

  // POST /modrinth/v0/servers/:id/name - Rename server
  if (method === 'POST' && /^\/modrinth\/v0\/servers\/[^/]+\/name$/.test(pathname)) {
    return empty()
  }

  // POST /modrinth/v0/servers/:id/subdomain - Change subdomain
  if (method === 'POST' && /^\/modrinth\/v0\/servers\/[^/]+\/subdomain$/.test(pathname)) {
    return empty()
  }

  // POST /modrinth/v0/servers/:id/notices/:id/dismiss
  if (
    method === 'POST' &&
    /^\/modrinth\/v0\/servers\/[^/]+\/notices\/\d+\/dismiss$/.test(pathname)
  ) {
    return empty()
  }

  // ── Archon v1 (/v1/...) ───────────────────────────────────────────────────

  // GET /v1/regions - List available regions
  if (method === 'GET' && /^\/v1\/regions$/.test(pathname)) {
    return json([MOCK_REGION])
  }

  // GET /v1/servers - List servers (v1 format)
  if (method === 'GET' && /^\/v1\/servers$/.test(pathname)) {
    return json([MOCK_SERVER_V1])
  }

  // GET /v1/servers/:id - Get server details (v1 format)
  if (method === 'GET' && /^\/v1\/servers\/[^/]+$/.test(pathname)) {
    return json(MOCK_SERVER_V1)
  }

  // DELETE /v1/servers/:id/flows/intro - Mark intro flow as complete
  if (method === 'DELETE' && /^\/v1\/servers\/[^/]+\/flows\/intro$/.test(pathname)) {
    return empty()
  }

  // GET /v1/servers/:id/worlds/:wid/properties - Get server.properties
  if (method === 'GET' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/properties$/.test(pathname)) {
    return json(MOCK_PROPERTIES)
  }

  // PATCH /v1/servers/:id/worlds/:wid/properties - Update server.properties
  if (method === 'PATCH' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/properties$/.test(pathname)) {
    return empty()
  }

  // GET /v1/servers/:id/worlds/:wid/options/startup
  if (
    method === 'GET' &&
    /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/options\/startup$/.test(pathname)
  ) {
    return json(MOCK_RUNTIME_OPTIONS)
  }

  // PATCH /v1/servers/:id/worlds/:wid/options/startup
  if (
    method === 'PATCH' &&
    /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/options\/startup$/.test(pathname)
  ) {
    return empty()
  }

  // GET /v1/servers/:id/worlds/:wid/addons - Get mods/addons
  if (method === 'GET' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/addons/.test(pathname)) {
    return json(MOCK_ADDONS)
  }

  // POST /v1/servers/:id/worlds/:wid/addons - Install addons
  if (method === 'POST' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/addons/.test(pathname)) {
    return empty()
  }

  // GET /v1/servers/:id/worlds/:wid/backups-queue
  if (
    method === 'GET' &&
    /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/backups-queue/.test(pathname)
  ) {
    return json(MOCK_BACKUPS_QUEUE)
  }

  // POST /v1/servers/:id/worlds/:wid/backups-queue - Create backup
  if (
    method === 'POST' &&
    /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/backups-queue$/.test(pathname)
  ) {
    return json({ id: 'mock-backup-' + Date.now() })
  }

  // DELETE /v1/servers/:id/worlds/:wid/backups-queue/:id
  if (
    method === 'DELETE' &&
    /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/backups-queue\/[^/]+$/.test(pathname)
  ) {
    return empty()
  }

  // POST /v1/servers/:id/worlds/:wid/content - Update content
  if (method === 'POST' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/content/.test(pathname)) {
    return empty()
  }

  // GET /v1/servers/:id/worlds/:wid/content/... (modpack preview, etc.)
  if (method === 'GET' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/content\//.test(pathname)) {
    return json(null)
  }

  // POST /v1/servers/:id/worlds/:wid/onboard - World onboarding
  if (method === 'POST' && /^\/v1\/servers\/[^/]+\/worlds\/[^/]+\/onboard$/.test(pathname)) {
    return empty()
  }

  return null
}

// ── Labrinth Billing Router ───────────────────────────────────────────────────

function routeBilling(method: string, pathname: string): Response | null {
  // GET /_internal/billing/customer - Get Stripe customer info
  if (method === 'GET' && pathname === '/_internal/billing/customer') {
    return json(MOCK_CUSTOMER)
  }

  // GET /_internal/billing/payment_methods - Get saved payment methods
  if (method === 'GET' && pathname === '/_internal/billing/payment_methods') {
    return json(MOCK_PAYMENT_METHODS)
  }

  // POST /_internal/billing/payment_method - Add new payment method
  if (method === 'POST' && pathname === '/_internal/billing/payment_method') {
    return json({
      client_secret: 'mock_client_secret',
      status: 'requires_action',
    })
  }

  // PATCH /_internal/billing/payment_method/:id
  if (method === 'PATCH' && /^\/_internal\/billing\/payment_method\/[^/]+$/.test(pathname)) {
    return empty()
  }

  // DELETE /_internal/billing/payment_method/:id
  if (method === 'DELETE' && /^\/_internal\/billing\/payment_method\/[^/]+$/.test(pathname)) {
    return empty()
  }

  // GET /_internal/billing/subscriptions - Get user's subscriptions
  if (method === 'GET' && pathname === '/_internal/billing/subscriptions') {
    return json([MOCK_SUBSCRIPTION])
  }

  // GET /_internal/billing/products - Get available products
  if (method === 'GET' && pathname === '/_internal/billing/products') {
    return json(MOCK_PRODUCTS)
  }

  // PATCH /_internal/billing/subscription/:id - Edit subscription
  if (method === 'PATCH' && /^\/_internal\/billing\/subscription\/[^/]+$/.test(pathname)) {
    return json({
      subscription: MOCK_SUBSCRIPTION,
      payment_intent: null,
    })
  }

  // GET /_internal/billing/payments - Get payment history
  if (method === 'GET' && pathname === '/_internal/billing/payments') {
    return json(MOCK_PAYMENTS)
  }

  // POST /_internal/billing/payment - Initiate a payment
  if (method === 'POST' && pathname === '/_internal/billing/payment') {
    return json({
      subscription: MOCK_SUBSCRIPTION,
      payment_intent: {
        id: 'pi_mock_' + Date.now(),
        client_secret: 'pi_mock_secret',
        status: 'succeeded',
      },
    })
  }

  // POST /_internal/billing/charge/:id/refund - Refund a charge
  if (method === 'POST' && /^\/_internal\/billing\/charge\/[^/]+\/refund$/.test(pathname)) {
    return empty()
  }

  // POST /_internal/billing/credit - Credit subscription
  if (method === 'POST' && pathname === '/_internal/billing/credit') {
    return empty()
  }

  return null
}

// ── User API Router ───────────────────────────────────────────────────────────

function routeUser(method: string, pathname: string): Response | null {
  // GET /v2/user - Get current user
  if (method === 'GET' && (pathname === '/v2/user' || pathname === '/user')) {
    return json(MOCK_USER)
  }

  // GET /v2/user/servers - Get user's servers (if this endpoint exists)
  if (method === 'GET' && pathname === '/v2/user/servers') {
    return json([MOCK_SERVER_V0])
  }

  return null
}

// ── Plugin ────────────────────────────────────────────────────────────────────

export default defineNuxtPlugin(() => {
  // Safety: never run in production
  if (import.meta.env.PROD) {
    console.log('[hosting-mock] Disabled in production')
    return
  }

  console.log('[hosting-mock] Initializing comprehensive API mock...')

  const config = useRuntimeConfig()

  // Build base URLs
  const archonBase = (config.public.pyroBaseUrl as string || 'https://staging-archon.modrinth.com')
    .replace(/\/v2\/?$/, '')
    .replace(/\/$/, '')

  const labrinthBase = (config.public.apiBaseUrl as string || 'https://staging-api.modrinth.com/v2/')
    .replace(/\/v2\/?$/, '')
    .replace(/\/$/, '')

  console.log(`[hosting-mock] Archon base: ${archonBase}`)
  console.log(`[hosting-mock] Labrinth base: ${labrinthBase}`)

  const originalFetch = globalThis.fetch.bind(globalThis)

  globalThis.fetch = async function mockFetch(
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> {
    const urlStr = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
    const method = (init?.method ?? 'GET').toUpperCase()

    try {
      const url = new URL(urlStr)
      const pathname = url.pathname

      // Intercept Archon API calls
      if (urlStr.startsWith(archonBase)) {
        console.log(`[hosting-mock] Archon: ${method} ${pathname}`)
        const response = routeArchon(method, pathname)
        if (response !== null) return response
        return notFound(`Archon ${pathname}`)
      }

      // Intercept Labrinth billing API calls
      if (urlStr.startsWith(labrinthBase) && pathname.startsWith('/_internal/billing')) {
        console.log(`[hosting-mock] Billing: ${method} ${pathname}`)
        const response = routeBilling(method, pathname)
        if (response !== null) return response
        return notFound(`Billing ${pathname}`)
      }

      // Intercept Labrinth user API calls
      if (urlStr.startsWith(labrinthBase) && (pathname === '/v2/user' || pathname === '/user')) {
        console.log(`[hosting-mock] User: ${method} ${pathname}`)
        const response = routeUser(method, pathname)
        if (response !== null) return response
      }

      // Fall through to real fetch for everything else
      return originalFetch(input, init)
    } catch (error) {
      // If URL parsing fails, just pass through to real fetch
      return originalFetch(input, init)
    }
  }

  console.log('[hosting-mock] Mock interceptor active - all hosting & billing APIs will return fake data')
  console.log('[hosting-mock] You can now browse the hosting UI without any backend!')
})
