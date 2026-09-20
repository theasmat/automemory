# Phase 2.5: Next-Gen React Dashboard & Modular WebSocket Engine

## Goals
1. Replace legacy vanilla dashboard with modern **React 19 + TypeScript + Vite + Tailwind CSS** architecture.
2. Provide **Bidirectional WebSocket (`/ws`)** transport on `automemory serve` with real-time event broadcasting and JSON-RPC dispatch.
3. Utilize **Web Workers** for off-thread compute:
   - `graphLayout.worker.ts` for force-directed AST & memory graph physics simulation at 60fps.
   - `eventIndexer.worker.ts` for off-thread log and live ledger filtering.
4. **Localhost First**: Direct local daemon connection by default, with remote tunnel capability.
5. **Zero Runtime Dependencies**: Embedded in standalone single-binary release via `rust-embed`.

## Directory Structure
- `crates/automemory-cli/src/commands/serve.rs`: Axum WebSocket endpoint, broadcast channel, static file serving.
- `clients/web/`: React 19 SPA source.
  - `src/workers/`: Web Worker scripts.
  - `src/services/`: WS client & RPC dispatcher.
  - `src/hooks/`: React state and worker hooks.
  - `src/store/`: Zustand reactive store.
  - `src/components/`: Modular UI views (Graph, Proposals, Memories, Traces, Failures, Context, Events).
