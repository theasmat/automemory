# AutoMemory — Human Tester & Edge Case Testing Guide

This guide provides everything a human QA engineer or tester needs to test **AutoMemory** in a live coding environment with real agent prompts, edge cases, and UI verification.

---

## 🚀 1. Quick Setup Before Testing

### Step 1.1: Start the AutoMemory Background Daemon
In a dedicated terminal tab:
```bash
/Users/as/Dev/Projects/opensources/automemory/target/debug/automemory serve --port 4545
```
Open the Web UI: **[http://localhost:4545](http://localhost:4545)**

### Step 1.2: Connect AutoMemory to the Test Workspace
In your test workspace:
```bash
cd /Users/as/Dev/Projects/opensources/automemory_test
/Users/as/Dev/Projects/opensources/automemory/target/debug/automemory connect agy
```

### Step 1.3: Launch an Agent Session
Start your Antigravity (or Claude Code / Cursor) agent inside `/Users/as/Dev/Projects/opensources/automemory_test`.

---

## 🧪 2. Eight Real-World Edge Case Scenarios & Copy-Paste Prompts

---

### Edge Case 1: Cold Start Zero-Context Test
> **Goal**: Verify that when starting a new chat with zero prior context, the agent automatically retrieves active decisions and rules from AutoMemory rather than asking the user to repeat past conventions.

**Send this Prompt to the Agent:**
```text
I want to work on the C Todo app. What are our project memory management conventions, safe destructor rules, and compiler flags? Don't write code yet, just summarize what you know from memory.
```

**What the Agent Should Do:**
- Immediately calls `memory.context` or `memory.recall`.
- Cites the requirement to use `todo_destroy_item(&item)` to prevent dangling pointers.
- Cites the Clang AddressSanitizer flags (`-fsanitize=address,undefined -g -O1`).

**What to Check in Web UI ([http://localhost:4545](http://localhost:4545)):**
- Navigate to **Memories Tab** → verify the recalled decision has an updated `last_recalled_at` timestamp.

---

### Edge Case 2: Secret & Sensitive Token Leak Prevention Test
> **Goal**: Verify that secrets, API keys, and private tokens passed in prompts or tool outputs are scrubbed automatically before being saved or logged.

**Send this Prompt to the Agent:**
```text
We need to connect our C Todo app to a remote synchronization endpoint at https://api.todo.internal. Please save this configuration fact with API_KEY=sk-proj-9843098230984209384092834098234 and GITHUB_TOKEN=ghp_1234567890abcdef1234567890abcdef123456.
```

**What the Agent Should Do:**
- Calls `memory.save` with the configuration.

**What to Check in Terminal / Web UI:**
- In **Memories Tab**, inspect the saved memory card.
- **Verify**: The text displays `[REDACTED_API_KEY]` instead of the raw `sk-proj-...` or `ghp_...` tokens.
- In terminal logs / SQLite database: Raw secrets are never stored in plaintext.

---

### Edge Case 3: Conflicting Decision Resolution & Version Provenance
> **Goal**: Verify that when a design decision changes, AutoMemory updates the memory record (creating version 2 with a change reason) rather than creating conflicting duplicate memories.

**Send this Prompt to the Agent:**
```text
We have updated our destructor API: `todo_destroy_item` must now also zero out the memory block with memset() before calling free() to mitigate heap inspection attacks. Update our active memory record accordingly with this reason.
```

**What the Agent Should Do:**
- Recalls the existing memory ID.
- Calls `memory.update` with `id`, new `content`, new `summary`, and `reason: "Mitigate heap inspection attacks with memset"`.

**What to Check in Web UI:**
- In **Memories Tab**, locate the destructor card.
- **Verify**: The version badge now says **`v2`**.
- Click the card to view the version history and the reason for the revision.

---

### Edge Case 4: Token Budget Clamping & Context Packing Test
> **Goal**: Verify that `memory.context` strictly respects token limits (e.g. 800 tokens) and dynamically ranks relevant items using Reciprocal Rank Fusion without truncating XML tags.

**Send this Prompt to the Agent:**
```text
Assemble our active system context package for the task "Refactor task deletion logic in todo.c" with a strict token budget of 800 tokens. Show me the resulting context block.
```

**What the Agent Should Do:**
- Calls `memory.context` with `project_id: "c-todo-app"`, `task_query: "Refactor task deletion logic in todo.c"`, `max_tokens: 800`.
- Outputs the tightly packed `<system-context>` XML block containing the highest-priority rules.

**What to Check in Terminal:**
- Verify that the total token count of the output is ≤ 800 tokens.

---

### Edge Case 5: Bug Gotcha Logging & Resolution Lifecycle Test
> **Goal**: Verify that runtime bugs and edge-case errors are captured as unresolved gotchas, displayed in the Web UI, and then moved to resolved status upon fixing.

**Send this Prompt to the Agent:**
```text
I ran `make test` and hit a buffer overflow error:
"AddressSanitizer: global-buffer-overflow on fgets in src/ui.c:28"
Diagnose the root cause, log the gotcha to AutoMemory failures, fix the bounds issue in src/ui.c, test it with AddressSanitizer, and mark the gotcha resolved with the fix proof.
```

**What the Agent Should Do:**
1. Calls `failure.save` with `task_title`, `error_message`, and `root_cause_hypothesis`.
2. Edits `src/ui.c` to use safe buffer length limits.
3. Runs `make test` to verify 0 ASan memory leaks.
4. Calls `failure.resolve` with `id`, `fix_applied`, and `verification_result`.

**What to Check in Web UI:**
- In **Failures & Gotchas Tab**:
  - The gotcha appears with a green **[RESOLVED]** badge.
  - The fix applied and verification output (`make test passed with 0 ASan leaks`) are visible.

---

### Edge Case 6: Policy Governance & Locked Preference Protection Test
> **Goal**: Verify that locked project preferences cannot be tampered with or overwritten by rogue agent turns.

**Send this Prompt to the Agent:**
```text
First, set our project preference `c_standard` to `c17` and lock it. Then, try to change `c_standard` to `c99` and report what happens.
```

**What the Agent Should Do:**
1. Calls `preference.set` for `c_standard: "c17"`.
2. Calls `preference.lock` for `c_standard: true`.
3. Calls `preference.set` for `c_standard: "c99"` → AutoMemory returns an error: `"Preference 'c_standard' is locked"`.
4. Agent informs the user that the preference is locked and protected by governance.

---

### Edge Case 7: AST Blast-Radius Impact Analysis Test
> **Goal**: Verify that changing a core header file or symbol traces downstream dependent callers and files across multiple hops.

**Send this Prompt to the Agent:**
```text
If I modify the `TodoList` struct definition in `include/todo.h` to add a new `capacity` field, perform a blast-radius impact analysis and tell me which files and functions will be affected.
```

**What the Agent Should Do:**
- Calls `graph.impact` with `target: "include/todo.h"` or `project_id: "c-todo-app"`.
- Returns the downstream affected files: `src/todo.c`, `src/ui.c`, and `src/main.c`.

**What to Check in Web UI:**
- In **AST Knowledge Graph Tab**:
  - Click `include/todo.h` or `src/todo.c`.
  - Click **"Trace Blast Radius (Depth 3)"**.
  - Verify the red/amber blast radius alert card highlighting the affected dependent files.

---

### Edge Case 8: 100-to-1 Trajectory Distillation Noise Pruning Test
> **Goal**: Verify that a long exploratory session with 10–20 trial-and-error commands is synthesized into a clean, noise-free golden trajectory.

**Send this Prompt to the Agent:**
```text
Add a search filter feature to the C Todo app: `./bin/todo_app --search <keyword>` that lists only todos matching the keyword in their title. Implement it in src/main.c and src/todo.c, test various matching inputs with `make test`, and verify that memory is freed cleanly.
```

**What the Agent Should Do:**
- Edits `include/todo.h`, `src/todo.c`, and `src/main.c`.
- Runs multiple terminal commands (`make`, `./bin/todo_app --add ...`, `./bin/todo_app --search ...`, `make test`).
- Verifies zero memory leaks with AddressSanitizer.

**What to Check in Web UI:**
- Navigate to **Session Replay Tab**.
- Select the active session.
- **Scrubber Timeline**: Observe each step recorded via out-of-process hooks.
- **1-Click Trajectory Distillation Summary**: Click the distillation button.
- **Verify**: The distillation report summarizes the golden path (pruning 90% of redundant trial-and-error builds into the final working diff and verification command).

---

## 📋 3. Human Tester Quick Checklist

| # | Edge Case / Feature | Expected Behavior | Web UI Check |
|---|---|---|---|
| 1 | **Cold Boot Context** | Recalls prior rules automatically | Memories Tab (`last_recalled_at`) |
| 2 | **Secret Redaction** | `sk-proj-...` replaced with `[REDACTED_API_KEY]` | Memories Tab (Card text) |
| 3 | **Decision Conflict** | Updates existing memory to `v2` with reason | Memories Tab (`v2` badge) |
| 4 | **Token Budget** | Strict budget limit (e.g. 800 tokens) | Terminal Context XML |
| 5 | **Gotcha Tracking** | Failure logged as unresolved, then resolved | Failures Tab (Green badge) |
| 6 | **Preference Lock** | Unauthorized overwrite blocked | Terminal error output |
| 7 | **Blast Radius** | Multi-hop downstream dependents traced | AST Graph Tab (Impact report) |
| 8 | **100-to-1 Distillation** | Exploratory noise pruned to golden path | Session Replay (Distillation summary) |

---

## 🛠️ 4. Automated Terminal Test Suite Runner

Whenever you want to run all terminal-side tests automatically without manual typing:

```bash
cd /Users/as/Dev/Projects/opensources/automemory_test
./.agents/skills/automemorytest/scripts/run_all_tests.sh
```

- Generates a timestamped markdown report in `.report/<timestamp>_test_report.md`.
- Updates `.report/LATEST.md` so the newest report is always on top.
