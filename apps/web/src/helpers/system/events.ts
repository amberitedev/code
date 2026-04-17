import { ref } from 'vue';

export type EventCallback<T = unknown> = (data: T) => void;

interface EventBus {
  on<T = unknown>(event: string, callback: EventCallback<T>): () => void;
  off<T = unknown>(event: string, callback: EventCallback<T>): void;
  emit<T = unknown>(event: string, data?: T): void;
}

type EventListeners = Map<string, Set<EventCallback>>;

const listeners: EventListeners = new Map();

function on<T = unknown>(event: string, callback: EventCallback<T>): () => void {
  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  listeners.get(event)!.add(callback as EventCallback);
  return () => off(event, callback);
}

function off<T = unknown>(event: string, callback: EventCallback<T>): void {
  listeners.get(event)?.delete(callback as EventCallback);
}

function emit<T = unknown>(event: string, data?: T): void {
  listeners.get(event)?.forEach((cb) => cb(data as T));
}

export const eventBus: EventBus = { on, off, emit };

export const ConnectionState = ref<'connected' | 'disconnected' | 'reconnecting'>('disconnected');

export const EVENTS = {
  AUTH_FAILED: 'auth-failed',
  CONNECTION_LOST: 'connection-lost',
  CONNECTION_RESTORED: 'connection-restored',
  NOTIFICATION_RECEIVED: 'notification-received',
  SYNC_COMPLETE: 'sync-complete',
  INSTANCE_STARTED: 'instance-started',
  INSTANCE_STOPPED: 'instance-stopped',
  INSTANCE_CRASHED: 'instance-crashed',
  FRIEND_ONLINE: 'friend-online',
  FRIEND_OFFLINE: 'friend-offline',
} as const;

export function authFailed(reason: string) {
  emit(EVENTS.AUTH_FAILED, { reason });
}

export function connectionLost(error: Error) {
  ConnectionState.value = 'disconnected';
  emit(EVENTS.CONNECTION_LOST, { error });
}

export function connectionRestored() {
  ConnectionState.value = 'connected';
  emit(EVENTS.CONNECTION_RESTORED, {});
}

export function notificationReceived(notification: import('../api/types').Notification) {
  emit(EVENTS.NOTIFICATION_RECEIVED, { notification });
}

export function syncComplete(instanceId: string, status: import('../api/types').SyncStatus) {
  emit(EVENTS.SYNC_COMPLETE, { instanceId, status });
}

export function instanceStarted(instanceId: string) {
  emit(EVENTS.INSTANCE_STARTED, { instanceId });
}

export function instanceStopped(instanceId: string) {
  emit(EVENTS.INSTANCE_STOPPED, { instanceId });
}

export function instanceCrashed(instanceId: string, exitCode: number) {
  emit(EVENTS.INSTANCE_CRASHED, { instanceId, exitCode });
}

export function friendOnline(userId: string, username: string) {
  emit(EVENTS.FRIEND_ONLINE, { userId, username });
}

export function friendOffline(userId: string, username: string) {
  emit(EVENTS.FRIEND_OFFLINE, { userId, username });
}

export function createEventHook<T = unknown>(event: string) {
  return {
    on: (callback: EventCallback<T>) => on(event, callback),
    off: (callback: EventCallback<T>) => off(event, callback),
  };
}