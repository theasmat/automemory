import { Hono } from 'hono';
import { cors } from 'hono/cors';
import type { Env } from './types';
import { healthRouter } from './routes/health';
import { devicesRouter } from './routes/devices';
import { syncRouter } from './routes/sync';
import { adminRouter } from './routes/admin';
import { billingRouter } from './routes/billing';

const app = new Hono<{ Bindings: Env }>();

// Enable CORS for web dashboard & agent clients
app.use(
  '*',
  cors({
    origin: '*',
    allowMethods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
    allowHeaders: ['Content-Type', 'Authorization', 'X-Admin-Secret'],
    exposeHeaders: ['Content-Length'],
    maxAge: 86400,
  })
);

// Mount routes
app.route('/', healthRouter);
app.route('/api/v1', devicesRouter);
app.route('/api/v1', syncRouter);
app.route('/api/v1', adminRouter);
app.route('/api/v1', billingRouter);


// 404 handler
app.notFound((c) => {
  return c.json({ error: 'NOT_FOUND', message: `Route ${c.req.path} not found` }, 404);
});

// Error handler
app.onError((err, c) => {
  console.error('Unhandled Gateway Error:', err);
  return c.json(
    {
      error: 'INTERNAL_SERVER_ERROR',
      message: err.message || 'An unexpected error occurred',
    },
    500
  );
});

export default app;
