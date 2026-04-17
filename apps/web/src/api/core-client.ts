import { supabase } from '@/plugins/supabase'

const CORE_BASE_URL = import.meta.env.VITE_CORE_URL || import.meta.env.VITE_API_BASE_URL || 'http://localhost:16662'

async function getJwt() {
  const { data } = await supabase.auth.getSession()
  return data.session?.access_token
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const token = await getJwt()
  
  if (!token) {
    throw new Error('Not authenticated')
  }

  const response = await fetch(`${CORE_BASE_URL}${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
    body: body ? JSON.stringify(body) : undefined,
  })

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: 'Request failed' }))
    throw new Error(error.error || error.message || 'Request failed')
  }

  return response.json()
}

export interface Instance {
  id: string
  name: string
}

export const coreApi = {
  async getInstances(): Promise<Instance[]> {
    const data = await request<{ instances: Instance[] }>('GET', '/instances')
    return data.instances
  },

  async startInstance(id: string): Promise<{ status: string }> {
    return request('POST', `/instances/${id}/start`)
  },

  async stopInstance(id: string): Promise<{ status: string }> {
    return request('POST', `/instances/${id}/stop`)
  },

  async killInstance(id: string): Promise<{ status: string }> {
    return request('POST', `/instances/${id}/kill`)
  },

  async sendCommand(id: string, command: string): Promise<{ status: string }> {
    return request('POST', `/instances/${id}/command`, { command })
  },

  consoleUrl(id: string, token: string): string {
    return `ws://${CORE_BASE_URL.replace(/^http(s)?:/, 'ws$1:')}/instances/${id}/console?token=${token}`
  },
}