const MODRINTH_OAUTH_URL = 'https://modrinth.com/_/oauth';
const STORAGE_KEY_MODRINTH_SESSION = 'LODESTONE_MODRINTH_SESSION';
const STORAGE_KEY_MODRINTH_USER = 'LODESTONE_MODRINTH_USER';

export interface ModrinthUser {
  id: string;
  username: string;
  avatar_url: string | null;
  email?: string;
}

export interface ModrinthSession {
  access_token: string;
  refresh_token?: string;
  expires_at: number;
  user: ModrinthUser;
}

export function getModrinthSession(): ModrinthSession | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_MODRINTH_SESSION);
    if (!raw) return null;
    const session = JSON.parse(raw) as ModrinthSession;
    if (session.expires_at < Date.now()) {
      localStorage.removeItem(STORAGE_KEY_MODRINTH_SESSION);
      localStorage.removeItem(STORAGE_KEY_MODRINTH_USER);
      return null;
    }
    return session;
  } catch {
    return null;
  }
}

export function setModrinthSession(session: ModrinthSession): void {
  localStorage.setItem(STORAGE_KEY_MODRINTH_SESSION, JSON.stringify(session));
  localStorage.setItem(STORAGE_KEY_MODRINTH_USER, JSON.stringify(session.user));
}

export function clearModrinthSession(): void {
  localStorage.removeItem(STORAGE_KEY_MODRINTH_SESSION);
  localStorage.removeItem(STORAGE_KEY_MODRINTH_USER);
}

export function getModrinthUser(): ModrinthUser | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_MODRINTH_USER);
    if (!raw) return null;
    return JSON.parse(raw) as ModrinthUser;
  } catch {
    return null;
  }
}

export function isModrinthLoggedIn(): boolean {
  return getModrinthSession() !== null;
}

const OAUTH_STATE_KEY = 'LODESTONE_OAUTH_STATE';

function generateState(): string {
  const array = new Uint8Array(16);
  crypto.getRandomValues(array);
  return Array.from(array, (b) => b.toString(16).padStart(2, '0')).join('');
}

export function initiateModrinthOAuth(): string {
  const state = generateState();
  sessionStorage.setItem(OAUTH_STATE_KEY, state);

  const params = new URLSearchParams({
    client_id: 'lodestone',
    redirect_uri: `${window.location.origin}/auth/callback`,
    scope: 'user:read profile:read',
    response_type: 'code',
    state,
  });

  const oauthUrl = `${MODRINTH_OAUTH_URL}?${params.toString()}`;
  window.location.href = oauthUrl;

  return state;
}

export async function handleModrinthCallback(code: string, state: string): Promise<ModrinthSession | null> {
  const savedState = sessionStorage.getItem(OAUTH_STATE_KEY);
  sessionStorage.removeItem(OAUTH_STATE_KEY);

  if (!savedState || savedState !== state) {
    throw new Error('Invalid OAuth state');
  }

  try {
    const response = await fetch('https://modrinth.com/_/api/oauth/token', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        grant_type: 'authorization_code',
        code,
        redirect_uri: `${window.location.origin}/auth/callback`,
        client_id: 'lodestone',
      }),
    });

    if (!response.ok) {
      throw new Error('Failed to exchange code for token');
    }

    const tokenData = await response.json();

    const userResponse = await fetch('https://modrinth.com/_/api/user', {
      headers: {
        Authorization: `Bearer ${tokenData.access_token}`,
      },
    });

    if (!userResponse.ok) {
      throw new Error('Failed to fetch user info');
    }

    const user = await userResponse.json() as ModrinthUser;

    const session: ModrinthSession = {
      access_token: tokenData.access_token,
      refresh_token: tokenData.refresh_token,
      expires_at: Date.now() + tokenData.expires_in * 1000,
      user: {
        id: user.id,
        username: user.username,
        avatar_url: user.avatar_url,
      },
    };

    setModrinthSession(session);
    return session;
  } catch (error) {
    console.error('Modrinth OAuth error:', error);
    return null;
  }
}
