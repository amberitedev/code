import { ref } from 'vue';
import { getApiKeyHeader, getKey } from '@/helpers/auth/key-store';
import type { ConsoleMessage } from './types';

export type WebSocketState = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface WebSocketManager {
  send: (data: string) => void;
  close: () => void;
  state: WebSocketState;
}

const MAX_RECONNECT_DELAY = 30000;
const INITIAL_RECONNECT_DELAY = 1000;

export function createConsoleWebSocket(instanceId: string): WebSocketManager {
  const state = ref<WebSocketState>('disconnected');
  let ws: WebSocket | null = null;
  let reconnectAttempts = 0;
  let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
  let destroyed = false;

  const storedKey = getKey();
  if (!storedKey) {
    state.value = 'error';
    return {
      send: () => {},
      close: () => {},
      get state() { return state.value; },
    };
  }

  const protocol = storedKey.host.startsWith('localhost') || storedKey.host === '127.0.0.1' ? 'ws' : 'wss';
  const url = `${protocol}://${storedKey.host}:${storedKey.port}/instances/${instanceId}/console?key=${encodeURIComponent(storedKey.key)}`;

  function connect() {
    if (destroyed) return;

    state.value = 'connecting';
    ws = new WebSocket(url);

    ws.onopen = () => {
      state.value = 'connected';
      reconnectAttempts = 0;
      window.dispatchEvent(new CustomEvent('lodestone-connection-restored'));
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data) as ConsoleMessage;
        window.dispatchEvent(new CustomEvent('lodestone-console-message', { detail: { instanceId, message: data } }));
      } catch {
        window.dispatchEvent(new CustomEvent('lodestone-console-message', {
          detail: { instanceId, message: { timestamp: Date.now(), line: event.data, type: 'stdout' as const } }
        }));
      }
    };

    ws.onerror = () => {
      state.value = 'error';
    };

    ws.onclose = () => {
      state.value = 'disconnected';
      if (!destroyed) {
        scheduleReconnect();
      }
    };
  }

  function scheduleReconnect() {
    if (destroyed) return;
    const delay = Math.min(INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttempts), MAX_RECONNECT_DELAY);
    reconnectAttempts++;
    reconnectTimeout = setTimeout(connect, delay);
  }

  function send(data: string) {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(data);
    }
  }

  function close() {
    destroyed = true;
    if (reconnectTimeout) {
      clearTimeout(reconnectTimeout);
    }
    if (ws) {
      ws.close();
      ws = null;
    }
    state.value = 'disconnected';
  }

  connect();

  return {
    send,
    close,
    get state() { return state.value; },
  };
}
