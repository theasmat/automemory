# AutoMemory Research Archive (`.report/research/`)

This directory houses technical research papers, ecosystem analyses, and architectural investigations supporting the design of AutoMemory.

---

## 📚 Research Papers & Reports

### 1. [Multi-Agent Coding Ecosystem Needs & Architecture](file:///Users/as/Developer/project/ai/cli/automemory/.report/research/multi_agent_ecosystem_needs.md)
* **Focus**: Why developers use multiple coding agents (Cursor, Claude Code, Antigravity, Aider, Devin) and the critical architectural capabilities needed to support them.
* **Core Topics**:
  - The Polyglot Agent Stack in 2025/2026.
  - The 6 Fatal Failure Modes (Context Islands, Collision Blindness, Handoff Friction, Invariant Inconsistency, Multi-Agent Attribution Void, Database Lockouts).
  - The 6-Tier Memory Architecture (Universal Constitution, Procedural Skills, Shared Blackboard, Time-Span Ledger, Baton Handoff Protocol, Ephemeral Scratchpad).
  - Concrete specifications for AutoMemory: Time-Span Progress Rollups, Shared Multi-Agent Blackboard, and the Baton Handoff Protocol.
  - Phased Implementation Roadmap.

---

## 🎯 Executive Takeaways for Next-Gen Planning

1. **Amnesia Across Agents is the #1 Multi-Agent Bottleneck**: When Claude Code discovers a bug fix in the terminal, Cursor in the IDE must immediately know about it without manual copy-pasting.
2. **Collision Prevention Requires a Live Blackboard**: When running parallel agents or worktrees, agents need awareness of which files other agents are actively touching.
3. **Passing the Baton (100:1 Distillation)**: Cross-agent handoffs require compact, structured packets ($<400$ tokens) of what was changed, what passed, and what failed—not 50k token raw terminal logs.
4. **Time-Span Activity Rollups**: Tracking editing velocity (files touched, edit counts), verified achievements (tests passed, commits), and failures (errors hit, fixes applied) across all agents over time windows.
