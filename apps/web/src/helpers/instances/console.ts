import { ref, onUnmounted } from 'vue';
import { createConsoleWebSocket, type WebSocketManager } from '@/helpers/api/websocket';
import type { ConsoleMessage } from '@/helpers/api/types';

export interface ConsoleState {
  messages: ConsoleMessage[];
  isConnected: boolean;
  instanceId: string | null;
}

export function createConsole(instanceId: string) {
  const messages = ref<ConsoleMessage[]>([]);
  const isConnected = ref(false);
  const currentInstanceId = ref(instanceId);

  let wsManager: WebSocketManager | null = null;

  function handleMessage(event: CustomEvent<{ instanceId: string; message: ConsoleMessage }>) {
    const { instanceId: msgInstanceId, message } = event.detail;
    if (msgInstanceId === currentInstanceId.value) {
      messages.value.push(message);
      if (messages.value.length > 10000) {
        messages.value = messages.value.slice(-5000);
      }
    }
  }

  function handleConnected() {
    isConnected.value = true;
  }

  function handleDisconnected() {
    isConnected.value = false;
  }

  window.addEventListener('lodestone-console-message', handleMessage as EventListener);
  window.addEventListener('lodestone-connection-restored', handleConnected);
  window.addEventListener('lodestone-connection-lost', handleDisconnected);

  wsManager = createConsoleWebSocket(instanceId);

  onUnmounted(() => {
    window.removeEventListener('lodestone-console-message', handleMessage as EventListener);
    window.removeEventListener('lodestone-connection-restored', handleConnected);
    window.removeEventListener('lodestone-connection-lost', handleDisconnected);
    wsManager?.close();
  });

  function sendCommand(command: string) {
    if (wsManager && isConnected.value) {
      wsManager.send(JSON.stringify({ type: 'command', command }));
    }
  }

  function clearMessages() {
    messages.value = [];
  }

  return {
    messages,
    isConnected,
    sendCommand,
    clearMessages,
  };
}
