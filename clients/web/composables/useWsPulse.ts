import { ref, onMounted, onUnmounted } from 'vue';

export interface WsRpcRequest {
  jsonrpc: '2.0';
  id: number;
  method: string;
  params?: Record<string, any>;
}

export type WsEventListener = (payload: any) => void;

export function useWsPulse() {
  const isConnected = ref(false);
  const listeners = ref<WsEventListener[]>([]);
  let socket: WebSocket | null = null;
  let reconnectTimer: any = null;

  function connect() {
    if (import.meta.server) return;
    if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
      return;
    }

    const host = window.location.host || '127.0.0.1:4545';
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${proto}//${host}/ws`;

    try {
      socket = new WebSocket(wsUrl);

      socket.onopen = () => {
        isConnected.value = true;
        if (reconnectTimer) {
          clearTimeout(reconnectTimer);
          reconnectTimer = null;
        }
        // Send initial sync requests
        sendRpc('stats', {});
        sendRpc('memories', { limit: 100 });
        sendRpc('sessions', { limit: 50 });
      };

      socket.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data);
          listeners.value.forEach((fn) => fn(parsed));
        } catch {
          // Ignore unparseable frames
        }
      };

      socket.onclose = () => {
        isConnected.value = false;
        socket = null;
        scheduleReconnect();
      };

      socket.onerror = () => {
        isConnected.value = false;
      };
    } catch {
      isConnected.value = false;
      scheduleReconnect();
    }
  }

  function scheduleReconnect() {
    if (reconnectTimer) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      connect();
    }, 3000);
  }

  function sendRpc(method: string, params: Record<string, any> = {}) {
    if (socket && socket.readyState === WebSocket.OPEN) {
      const payload: WsRpcRequest = {
        jsonrpc: '2.0',
        id: Date.now(),
        method,
        params,
      };
      socket.send(JSON.stringify(payload));
    }
  }

  function onMessage(callback: WsEventListener) {
    listeners.value.push(callback);
    return () => {
      listeners.value = listeners.value.filter((l) => l !== callback);
    };
  }

  function disconnect() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (socket) {
      socket.close();
      socket = null;
    }
    isConnected.value = false;
  }

  return {
    isConnected,
    connect,
    disconnect,
    sendRpc,
    onMessage,
  };
}
