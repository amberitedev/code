/**
 * Amberite Configuration
 * 
 * Reads environment variables from import.meta.env (Vite)
 * Variables must start with VITE_ to be exposed to frontend
 */

/// Modrinth API endpoints
export const MODRINTH_API_URL = import.meta.env.VITE_MODRINTH_API_URL;
export const MODRINTH_API_URL_V3 = import.meta.env.VITE_MODRINTH_API_URL_V3;
export const MODRINTH_URL = import.meta.env.VITE_MODRINTH_URL;
export const MODRINTH_SOCKET_URL = import.meta.env.VITE_MODRINTH_SOCKET_URL;

/// Amberite endpoints
export const AMBERITE_API_URL = import.meta.env.VITE_AMBERITE_API_URL;
export const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL;
export const SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY;

/// Feature flags
export const IS_DEV = import.meta.env.DEV;
export const IS_PROD = import.meta.env.PROD;

/// App info
export const APP_NAME = 'Amberite';
export const APP_VERSION = '0.1.0';
