import type { PulseMessage, WsRpcResponse } from '../types';

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error';
export type PulseCallback = (pulse: PulseMessage) => void;
export type BroadcastCallback = (event: Record<string, unknown>) => void;

class WebSocketClient {
  private ws: WebSocket | null = null;
  private url: string;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 30;
  private reconnectTimer: number | null = null;
  private pingIntervalTimer: number | null = null;
  private pendingRequests = new Map<string, { resolve: (val: any) => void; reject: (err: any) => void; timer: number }>();
  private requestIdCounter = 0;
  private pulseListeners = new Set<PulseCallback>();
  private broadcastListeners = new Set<BroadcastCallback>();
  private stateChangeListeners = new Set<(state: ConnectionState) => void>();
  private latencyListeners = new Set<(latencyMs: number) => void>();

  public state: ConnectionState = 'disconnected';
  public lastLatencyMs: number = 0;

  constructor(customUrl?: string) {
    if (customUrl) {
      this.url = customUrl;
    } else {
      const isHttps = typeof window !== 'undefined' && window.location.protocol === 'https:';
      const host = typeof window !== 'undefined' && window.location.host ? window.location.host : '127.0.0.1:4545';
      const protocol = isHttps ? 'wss:' : 'ws:';
      this.url = `${protocol}//${host}/ws`;
    }
  }

  public setUrl(newUrl: string) {
    if (this.url !== newUrl) {
      this.url = newUrl;
      this.reconnect();
    }
  }

  public getUrl(): string {
    return this.url;
  }

  public connect() {
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      return;
    }

    this.setState('connecting');

    try {
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        this.reconnectAttempts = 0;
        this.setState('connected');
        this.startHeartbeat();
      };

      this.ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);

          // 1. Is it an RPC response?
          if (data && ('id' in data) && data.id !== null) {
            const reqId = String(data.id);
            const pending = this.pendingRequests.get(reqId);
            if (pending) {
              window.clearTimeout(pending.timer);
              this.pendingRequests.delete(reqId);
              if (data.error) {
                pending.reject(new Error(data.error));
              } else {
                pending.resolve(data.result);
              }
              return;
            }
          }

          // 2. Is it a pulse or broadcast notification?
          if (data && data.type === 'pulse') {
            this.pulseListeners.forEach((fn) => fn(data as PulseMessage));
          } else if (data) {
            this.broadcastListeners.forEach((fn) => fn(data));
          }
        } catch {
          // ignore non-json frames
        }
      };

      this.ws.onclose = () => {
        this.cleanup();
        this.setState('disconnected');
        this.scheduleReconnect();
      };

      this.ws.onerror = () => {
        this.setState('error');
      };
    } catch {
      this.setState('error');
      this.scheduleReconnect();
    }
  }

  public reconnect() {
    this.cleanup();
    if (this.reconnectTimer) {
      window.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.connect();
  }

  public disconnect() {
    this.cleanup();
    if (this.reconnectTimer) {
      window.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setState('disconnected');
  }

  public async call<T = unknown>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      // If disconnected, try reconnecting then throw
      this.connect();
      throw new Error(`WebSocket is not connected (state: ${this.state})`);
    }

    const id = `req_${++this.requestIdCounter}_${Date.now()}`;
    const payload = JSON.stringify({ id, method, params });

    return new Promise<T>((resolve, reject) => {
      const timer = window.setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error(`WebSocket RPC call timeout for method: ${method}`));
        }
      }, 10000);

      this.pendingRequests.set(id, { resolve, reject, timer });
      this.ws!.send(payload);
    });
  }

  private startHeartbeat() {
    if (this.pingIntervalTimer) clearInterval(this.pingIntervalTimer);
    this.pingIntervalTimer = window.setInterval(async () => {
      if (this.state === 'connected') {
        const start = performance.now();
        try {
          await this.call('ping');
          const elapsed = Math.round(performance.now() - start);
          this.lastLatencyMs = elapsed;
          this.latencyListeners.forEach((fn) => fn(elapsed));
        } catch {
          // ignore ping drop
        }
      }
    }, 5000);
  }

  private scheduleReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      return;
    }
    const delay = Math.min(1000 * Math.pow(1.5, this.reconnectAttempts), 10000);
    this.reconnectAttempts++;
    this.reconnectTimer = window.setTimeout(() => {
      this.connect();
    }, delay);
  }

  private cleanup() {
    if (this.pingIntervalTimer) {
      clearInterval(this.pingIntervalTimer);
      this.pingIntervalTimer = null;
    }
    for (const [, req] of this.pendingRequests) {
      window.clearTimeout(req.timer);
      req.reject(new Error('WebSocket connection closed'));
    }
    this.pendingRequests.clear();
  }

  private setState(newState: ConnectionState) {
    if (this.state !== newState) {
      this.state = newState;
      this.stateChangeListeners.forEach((fn) => fn(newState));
    }
  }

  public onStateChange(callback: (state: ConnectionState) => void) {
    this.stateChangeListeners.add(callback);
    callback(this.state);
    return () => this.stateChangeListeners.delete(callback);
  }

  public onLatency(callback: (latencyMs: number) => void) {
    this.latencyListeners.add(callback);
    return () => this.latencyListeners.delete(callback);
  }

  public onPulse(callback: PulseCallback) {
    this.pulseListeners.add(callback);
    return () => this.pulseListeners.delete(callback);
  }

  public onBroadcast(callback: BroadcastCallback) {
    this.broadcastListeners.add(callback);
    return () => this.broadcastListeners.delete(callback);
  }
}

export const wsClient = new WebSocketClient();
