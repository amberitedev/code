/**
 * Environment-specific configuration
 * 
 * This file exports environment variables with fallbacks.
 * In development, these can be overridden via .env files.
 */

/// Modrinth API endpoints
export const MODRINTH_API_URL = getEnvVar('VITE_MODRINTH_API_URL', 'https://api.modrinth.com/v2/');
export const MODRINTH_API_URL_V3 = getEnvVar('VITE_MODRINTH_API_URL_V3', 'https://api.modrinth.com/v3/');
export const MODRINTH_URL = getEnvVar('VITE_MODRINTH_URL', 'https://modrinth.com/');
export const MODRINTH_SOCKET_URL = getEnvVar('VITE_MODRINTH_SOCKET_URL', 'wss://api.modrinth.com/ws/');

/// Amberite-specific endpoints
export const AMBERITE_API_URL = getEnvVar('VITE_AMBERITE_API_URL', 'http://localhost:3000/');
export const SUPABASE_URL = getEnvVar('VITE_SUPABASE_URL', '');
export const SUPABASE_ANON_KEY = getEnvVar('VITE_SUPABASE_ANON_KEY', '');

/// Feature flags
export const IS_DEV = getEnvVar('DEV', 'false') === 'true';
export const IS_PROD = getEnvVar('PROD', 'false') === 'true';

/// App config
export const APP_NAME = 'Amberite';
export const APP_VERSION = '0.1.0';

/// Helper to get environment variables with fallbacks
function getEnvVar(name: string, fallback: string): string {
  // @ts-ignore - import.meta.env may not be defined in all environments
  const value = import.meta?.env?.[name];
  return value !== undefined ? value : fallback;
}
