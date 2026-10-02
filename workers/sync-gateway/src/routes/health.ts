import { Hono } from 'hono';
import type { Env } from '../types';

export const healthRouter = new Hono<{ Bindings: Env }>();

healthRouter.get('/health', (c) => {
  return c.json({
    status: 'ok',
    service: 'AutoMemory Pro Cloud Sync Gateway',
    edge: 'cloudflare-workers',
    timestamp: new Date().toISOString(),
  });
});
