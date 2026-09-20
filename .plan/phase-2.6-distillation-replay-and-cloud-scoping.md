# Phase 2.6: Session Trajectory Distillation, Interactive Replay & Cloud Scoping

## Strategic Objective
Transform AutoMemory Pro from a simple memory store into an intelligent **trajectory distillation and session replay platform** that outperforms standard agent memory systems:
1. **100 Tool Logs -> 1 Clean Distilled Memory**: Evaluates entire agent sessions, prunes redundant reads and noise, isolates pivotal fixes, and produces 1 clean reproducible trace and 1 high-confidence crystallized memory.
2. **Interactive Web Session Replay**: Step-by-step visual timeline scrubber with health heatmaps, execution duration, parameters, and distillation inspector.
3. **Local vs Cloud Memory Scoping**: Clear visual and architectural separation between local project-bound memories and cloud-synced team memories, with automatic project isolation.

## Key Components
- `crates/automemory-core/src/extraction/distiller.rs`: Core distillation engine.
- `crates/automemory-cli/src/commands/serve.rs`: WebSocket JSON-RPC session & distillation endpoints.
- `clients/web/src/components/replay/SessionReplayView.tsx`: Timeline player and action inspector.
- `clients/web/src/components/memories/MemoriesView.tsx`: Local vs Cloud filter and sync badges.
