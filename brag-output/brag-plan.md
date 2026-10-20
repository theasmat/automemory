# Brag Plan: AutoMemory

## What is this app?
Shared, verified memory, worktree advisory locks, and context baton handoffs for coding agent swarms — built in pure Rust as a single zero-daemon binary with local SQLite WAL.

## The angle
Autonomous coding agents without coordination are expensive amnesiacs: they overwrite each other's work across git worktrees and hallucinate destructive commands. AutoMemory gives multi-agent teams real-time worktree collision prevention, sub-2ms AST safety guardrails, and token-budgeted baton handoffs so 20 agents can code together without disaster.

## Hook (first 2-3 seconds)
A stark terminal prompt flashes the warning: `⚠️ SWARM COLLISION: 3 agents writing to same worktree`. Then with a crisp keypress, `automemory connect all` executes and instantly binds 20 agent harnesses into a single synchronized state.

## Key moments (the middle)
- **Sub-2ms AST Guardrail Intercept:** Agent attempts destructive command (`rm -rf /` or forced branch push) — intercepted and denied in 1.4ms with zero LLM latency overhead.
- **Cross-Worktree Advisory Leases:** Agent 1 locks `src/engine.rs` in `feature/auth` worktree; Agent 2 in `feature/billing` gets immediate collision block with lock holder telemetry.
- **Zero-Staleness Baton Handoff:** Compact token-budgeted handoff context passed between Claude, Codex, and Cursor with verified state hash.

## Outro / punchline
"Coding agents don't need another cloud subscription. They need memory."
AutoMemory. Pure Rust. Single binary. Local SQLite WAL.

## User flow worth showing
1. Agent attempts destructive execution or concurrent file write.
2. AutoMemory intercept hook fires in `<2ms`: guardrail AST check blocks `rm -rf /` and signals `BLOCKED: Policy Violation`.
3. Agent 1 acquires advisory lease on `src/engine.rs` across git worktrees; Agent 2 detects lease collision and waits.
4. Agent 1 finishes, passes context baton with delta state, and Agent 2 seamlessly resumes.

## Tone
- Preset: `polished`
- Creative direction: "Hyper-focused systems engineering film for multi-agent swarm infrastructure"
- Interpretation: Serious, punchy, confident, and high-tech. Dark slate background with glowing gold accents and high-contrast terminal code elements. Fast, crisp cuts with generous holds on readable data.

## Format: landscape — 1920x1080
## Duration: 19.0s

## Visual identity (from the project)
- Background: `#0b0c0e` (dark obsidian)
- Accent: `#ffd043` (warm gold / amber)
- Border / Surface: `#1b1e24` and `#272b35`
- Text primary: `#f3f4f6`
- Text muted: `#9ca3af`
- Status Green: `#10b981` (ok / lease active)
- Status Red: `#ef4444` (intercept blocked)
- Display font: `Inter`, sans-serif
- Code font: `JetBrains Mono`, monospace
- Strongest visual element: Terminal HUD with glowing status badges (`INTERCEPTED <1.4ms`, `LEASE ACQUIRED`, `BATON SYNCED`).

## Share copy (draft)
Coding agents without memory are just expensive amnesiacs.
AutoMemory gives 20 agent harnesses verified shared memory, sub-2ms AST guardrails, and zero-collision worktree locks.
Built in pure Rust. Single binary. Local SQLite WAL.

## Audio direction
- Role: Sparse, driving electronic tech bed with crisp UI hits and heavy impact accents.
- Music: `happy-beats-business-moves-vol-11-by-ende-dot-app.mp3` (Tempo: 114.84 BPM).
- Music treatment: Starts at full presence (0.75 volume), subtle swell into mid-scene, gradual duck under punchline, sharp hit at final logo reveal.
- Music cue guidance:
  - Strong cues: 1.60s (hook reveal), 3.70s (guardrail intercept), 5.80s (policy block slam), 12.65s (baton transfer complete), 17.91s (logo impact).
  - Beat-grid windows: 8.44s – 11.60s for sequential agent lock status steps (0.52s spacing).
- Audio-reactive treatment: Subtle; hero badge glow and terminal border intensity breathe with RMS bass energy.
- SFX posture: Moderate, crisp, motion-matched:
  - Keystroke clicks for CLI command.
  - Metallic impact / glitch on guardrail intercept.
  - Card slide / toggle switch on worktree lease acquisition.
  - Low resonant bell on final logo lock.
- Restraint rule: No cartoonish sounds, no vocal samples, no chaotic sweeps.

---

## Storyboard

### Scene 1 — The Amnesiac Swarm Hook — 3.5s (0.0s – 3.5s)
- **Visual:** Pitch-black canvas `#0b0c0e`. A high-contrast terminal window displays red alert: `⚠️ SWARM COLLISION: 3 agents writing to same files`.
- Fast command types out: `automemory connect all` with prompt `$`.
- Instant green cascade: `[OK] Connected 20 agent harnesses to local SQLite WAL`.
- **Sequential / Interaction:** Terminal types command at 0.8s, hits enter at 1.60s (`// beat-locked: 1.60s`), status badges cascade in.
- **Audio intent:** Tension followed by immediate control and relief.
- **Audio-coupled idea:** Subtle keyboard ticks during typing, crisp `click_002.ogg` on enter, heavy hit `impactSoft_medium_000.ogg` on 1.60s cue.
- **Transition mood:** Hard snap → Scene 2

### Scene 2 — Sub-2ms Guardrail Intercept — 4.5s (3.5s – 8.0s)
- **Visual:** Split-view terminal monitor. Left: Agent attempt `exec("rm -rf /")`. Right: AutoMemory Safety Interceptor badge flashes bright red: `BLOCKED [AST Guardrail]`.
- Telemetry counter rapidly settles at `1.4 ms latency`.
- Callout banner: `Zero LLM overhead. Pure Rust AST inspection.`
- **Sequential / Interaction:** Command executes at 4.2s, intercepted at 5.80s (`// beat-locked: 5.80s`), latency badge pops.
- **Audio intent:** Sudden interception, mechanical precision.
- **Audio-coupled idea:** Metal glitch / plate click `impactPlate_light_002.ogg` on intercept trigger.
- **Transition mood:** Clean slide → Scene 3

### Scene 3 — Worktree Advisory Locks & Baton Handoff — 6.5s (8.0s – 14.5s)
- **Visual:** Worktree matrix dashboard showing two parallel git branches:
  - Worktree A (`feature/auth`): Agent 1 acquires exclusive lease on `src/engine.rs` (`[LEASE ACQUIRED: ttl 120s]`).
  - Worktree B (`feature/billing`): Agent 2 attempts edit $\rightarrow$ `COLLISION PREVENTED: Locked by Agent-1`.
  - Baton pass animation: Token-budgeted context packet flows from Agent 1 to Agent 2: `[BATON PASSED: 4.2KB delta]`.
- **Sequential / Interaction:**
  - Beat-grid step 1 (8.96s): Worktree A lock card slides in (`// beat-grid: 8.96s`).
  - Beat-grid step 2 (10.01s): Worktree B collision warning flashes amber (`// beat-grid: 10.01s`).
  - Beat-grid step 3 (12.65s): Baton transfer completes with green check (`// beat-locked: 12.65s`).
- **Audio intent:** Smooth coordinated rhythm, swarm harmony.
- **Audio-coupled idea:** Card slide sound `card-slide-1.ogg` on lease card, switch click on baton transfer.
- **Transition mood:** Dramatic wipe → Scene 4

### Scene 4 — Punchline & Architecture Outro — 4.5s (14.5s – 19.0s)
- **Visual:** The AutoMemory brand lockup:
  - Big bold typography: `AUTOMEMORY` in gold `#ffd043` with glowing ambient bloom.
  - Subtitle: `Shared Memory & Worktree Locks for Coding Agents`.
  - Feature badges: `Pure Rust` • `Single Binary` • `Local SQLite WAL` • `Zero Cloud Daemon`.
  - Punchline: `Stop amnesiac swarms. Give your agents memory.`
- **Sequential / Interaction:** Logo slams at 15.5s, feature pill tags pop at 16.5s, punchline settles at 17.5s, hold until 19.0s.
- **Audio intent:** Triumphant, decisive, grounded authority.
- **Audio-coupled idea:** Deep resonant impact bell `impactBell_heavy_000.ogg` at 15.81s / 17.91s, music ducks smoothly to silence at 19.0s.

---
**Music mood for this video:** Driving modern tech electronic (114.8 BPM)
**Audio summary:** Rhythmic high-tech pulse that punctuates CLI execution, slams on safety interception, synchronizes worktree leases, and lands on a resonant logo lockup.
