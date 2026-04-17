import { ref, onUnmounted } from 'vue';
import { apiClient } from '@/helpers/api/client';
import type { Presence } from '@/helpers/api/types';

const PRESENCE_POLL_INTERVAL = 30000;

export interface PresenceState {
  friends: Presence[];
  lastUpdated: Date | null;
  isLoading: boolean;
}

export function createPresenceTracker() {
  const friends = ref<Presence[]>([]);
  const lastUpdated = ref<Date | null>(null);
  const isLoading = ref(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  async function fetchPresence() {
    isLoading.value = true;
    try {
      const data = await apiClient<Presence[]>('/api/friends/presence', {
        method: 'GET',
      });
      friends.value = data;
      lastUpdated.value = new Date();
    } catch (error) {
      console.error('Failed to fetch presence:', error);
    } finally {
      isLoading.value = false;
    }
  }

  function startPolling() {
    fetchPresence();
    pollInterval = setInterval(fetchPresence, PRESENCE_POLL_INTERVAL);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  onUnmounted(() => {
    stopPolling();
  });

  return {
    friends,
    lastUpdated,
    isLoading,
    fetchPresence,
    startPolling,
    stopPolling,
  };
}
