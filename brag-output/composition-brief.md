# Hyperframes Composition Brief: AutoMemory

## Objective
Create a short, high-energy, polished launch brag video for AutoMemory.

## Output
- Composition directory: `brag-output/composition/`
- Rendered video: `brag-output/brag.mp4`
- Format: landscape — 1920x1080
- Duration: 19.0s (within the 15-25s strict limit)

## Source Material
- Project root: `/Users/as/Developer/project/ai/cli/automemory`
- Primary files read: `README.md`, `crates/cli/src/main.rs`, `crates/guardrail/src/lib.rs`, `crates/blackboard/src/lib.rs`, `crates/worktree/src/lib.rs`, `clients/web/pages/brain.vue`
- Product name: AutoMemory
- Tagline / strongest claim: "Shared, verified memory, worktree advisory locks, and context baton handoffs for coding agent swarms. Pure Rust, single binary, local SQLite."
- Key UI or visual moment to recreate:
  - Terminal CLI executing `automemory connect all` and connecting 20 agent harnesses.
  - Sub-2ms AST safety guardrail intercepting `rm -rf /` in 1.4ms with zero LLM overhead.
  - Multi-agent cross-worktree lease collision alert and baton context handoff.
- Copy that must appear verbatim:
  - `automemory connect all`
  - `BLOCKED [AST Guardrail]`
  - `< 1.4ms Latency`
  - `Worktree Advisory Lease: Locked by Agent-1`
  - `Baton Context Handoff: Synchronized`
  - `Pure Rust • Single Binary • Local SQLite WAL`

## Creative Direction
- Tone preset: `polished`
- Creative direction: "Hyper-focused systems engineering film for multi-agent swarm infrastructure"
- Interpretation: Serious, punchy, confident, and high-tech. Dark slate obsidian palette with radiant amber/gold accents. Crisp micro-interactions, high data density, no generic SaaS fluff.
- Angle: Autonomous coding agents without coordination are expensive amnesiacs: they overwrite each other's work across git worktrees and hallucinate destructive commands. AutoMemory gives multi-agent teams real-time worktree collision prevention, sub-2ms AST safety guardrails, and token-budgeted baton handoffs so 20 agents can code together without disaster.
- Hook: A terminal flashes `⚠️ SWARM COLLISION: 3 agents writing to same worktree`, then `automemory connect all` instantly binds 20 agent harnesses into synchronized harmony.
- Outro / punchline: "Stop amnesiac swarms. Give your agents memory."

## Visual Identity
- Background: `#0b0c0e` (obsidian canvas)
- Card / Panel: `#14171d` with `#232730` subtle borders
- Accent Gold: `#ffd043`
- Accent Amber: `#f59e0b`
- Status Green: `#10b981` (ok / lease active)
- Status Red: `#ef4444` (policy violation / blocked)
- Text primary: `#f3f4f6`
- Text muted: `#9ca3af`
- Display font: `Inter`, system-ui, sans-serif
- Code font: `JetBrains Mono`, `Fira Code`, monospace

## Storyboard
Use the storyboard in `brag-output/brag-plan.md` as the creative contract:
1. **Scene 1 (0.0s – 3.5s)**: The Amnesiac Swarm Hook. Terminal window shows collision alert. `$ automemory connect all` executes at 1.60s (`// beat-locked: 1.60s`), lighting up 20 connected agent harnesses.
2. **Scene 2 (3.5s – 8.0s)**: Sub-2ms AST Guardrail Intercept. Command `rm -rf /` executed at 4.2s. Intercepted with red badge `BLOCKED [AST Guardrail]` at 5.80s (`// beat-locked: 5.80s`). Latency meter shows `1.4ms`.
3. **Scene 3 (8.0s – 14.5s)**: Worktree Locks & Baton Handoff. Dual worktree panels:
   - Worktree A (`feature/auth`): `src/engine.rs` leased by Agent 1 (`// beat-grid: 8.96s`).
   - Worktree B (`feature/billing`): Agent 2 attempt blocked (`// beat-grid: 10.01s`).
   - Baton transfer passes token-budgeted state delta at 12.65s (`// beat-locked: 12.65s`).
4. **Scene 4 (14.5s – 19.0s)**: Punchline & Architecture Outro. Hero branding `AUTOMEMORY` in glowing gold. Badges: `Pure Rust` • `Single Binary` • `Local SQLite WAL` • `Zero Cloud Daemon`. Punchline: "Stop amnesiac swarms. Give your agents memory."

## Audio
- Audio role: Sparse, driving electronic tech bed with crisp UI hits and impact accents.
- Audio arc: Fast rhythmic build into mechanical precision, climaxing on synchronized handoff and deep resonant outro bell.
- Music: `happy-beats-business-moves-vol-11-by-ende-dot-app.mp3` (Tempo: 114.84 BPM).
- Music treatment: Starts at 0.75 volume, holds presence throughout, smooth fade-out between 18.0s and 19.0s.
- Music cue guidance:
  - Strong cues: 1.60s (hook reveal), 5.80s (guardrail block), 12.65s (baton synced), 17.91s (outro logo accent).
  - Beat grid: 8.96s, 10.01s, 11.06s for sequential steps.
- Audio-reactive treatment: Subtle glow and border pulsation responding to music energy.
- Audio-coupled moments:
  - 1.60s: CLI Enter & swarm connection (`impactSoft_medium_000.ogg`).
  - 5.80s: Guardrail policy intercept (`impactPlate_light_002.ogg`).
  - 8.96s: Advisory lease card entrance (`card-slide-1.ogg`).
  - 12.65s: Baton sync confirmation (`click_002.ogg`).
  - 15.80s: Logo reveal resonance (`impactBell_heavy_000.ogg`).
- Audio files: Copy music and selected SFX into `brag-output/composition/assets/`.

## Hyperframes Instructions
Build a self-contained, high-performance HTML/CSS/JS composition that complies with Hyperframes standards:
- All assets referenced relatively (`assets/...`).
- Validated with `npx hyperframes check`.
- Verified contrast and layout safety.
- Strict 19.0s timeline.
