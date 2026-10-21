# Project library: agent documents that are not in git, kept in sync by AutoMemory

Status: plan only (Oct 2026). Nothing in this file is built. Not part of v1 scope; see [docs/PLAN.md §17](../docs/PLAN.md).

## 1. The need
PRDs, plans, research notes, scratch specs, prompts and "read this first" files help agents, but people do not want them in the
repository: they clutter `git status`, leak into pull requests, differ per person, or are private. Today they live in random
folders, are lost on another computer, and each agent finds them only if told. We want:

- files that are **not tracked by git** and never show in `git status`,
- **synced between your computers** (and optionally teammates) like the rest of AutoMemory: encrypted, through the vault,
- **found by every agent** (MCP, hook index, and plain file paths for agents without MCP),
- **cheap in tokens**: an index and short abstracts first, the full file on demand,
- **safe**: secrets blocked, files treated as data, agents cannot silently rewrite what you wrote.

Today only in-repo documents are tracked (`docs/scanner.rs` reads `PRD.md`, `docs/**/*.md`, ... inside the repository). This adds the other half.

## 2. Idea taken from OpenViking (and what we do not take)
[OpenViking](https://github.com/volcengine/OpenViking) (Volcengine, open sourced Jan 2026) is a "context database for agents": memories,
resources and skills in one virtual filesystem addressed by `viking://` paths; every entry has three tiers (abstract, overview,
details) so an agent loads only as deep as needed; retrieval finds the best directory first and then drills down. Sources:
[PyPI page](https://pypi.org/project/openviking), [README](https://cdn.jsdelivr.net/gh/volcengine/OpenViking@main/README.md),
[docs](https://www.mintlify.com/volcengine/OpenViking/introduction). Details such as the token targets per tier are from secondary
write-ups; check the official docs before copying numbers.

| We take | We do not take |
|---|---|
| Files addressed by a stable path, browsed with list/tree/find | A separate Python service and model-built abstracts at write time (our rule: no LLM in the daemon; abstracts are deterministic, a model may only suggest) |
| Three tiers: L0 title and one line, L1 outline and key points, L2 the whole file | Their storage; ours stays SQLite plus plain files plus the existing encrypted vault |
| Directory-first search, then drill down | Replacing our memory/brief/chat archive: the library is one more source beside them |

## 3. User experience
```
automemory library add ~/Desktop/PRD-cart.md --as plans/prd-cart.md   copy in (add --move to remove the original)
automemory library new notes/api-quirks.md                            create and open in $EDITOR
automemory library ls | tree | show plans/prd-cart.md | search "refund"
automemory library sync --push|--pull                                  or automatic, see 6
```
- **Where the files are:** `~/.automemory/library/<project>/...` is the source of truth (plain files you can edit with any tool).
- **Where agents see them:** a read-only-by-convention folder in the project, `.automemory/library/` (symlink on macOS and Linux,
  copy on Windows), excluded through **`.git/info/exclude`** (local to the clone, never committed, never touches `.gitignore`).
  `git status` stays clean; a test proves it. An `INDEX.md` in that folder lists every file with its L0 line.
- **Agents with MCP:** `library.list`, `library.get {path, tier}`, `library.search {query}`, `library.put {path, content}`.
- **Hooks:** the session-start pack adds a short "Project library" index (L0 lines only, within the token budget); the rules text
  tells agents to open `.automemory/library/INDEX.md` or call `library.get`.
- **Dashboard (Library page):** tree, preview at each tier, new/edit/upload (Markdown editor), history and revert, "where agents
  see it", sync state, and a banner if the folder is not excluded from git.

## 4. Data and rules
- **Entry:** path (unique per project), kind (md, txt, json, yaml; images and PDFs later), size, sha256, revisions, origin
  (`human` or `agent:<name>`), author, updated_at, tiers (`l0`, `l1` generated), scope (`project` default, `personal` = this machine only).
- **Limits:** 1 MB per file, 50 MB per project, text only in the first version.
- **Secrets:** files go through the same scan as notes (`contains_secrets`/`redact_text`). A file with a secret is **refused** (not
  silently altered) with the line number; the human can override per file, never an agent.
- **Who can change what:** human files are changed by humans (dashboard needs the password, CLI as today); agents may create files
  and edit files they created; `library.put` on a human file returns a refusal and offers `library.propose` (a suggested edit that
  appears as a diff in the Inbox). Same pattern as brief items.
- **Library text is data:** it is shown to agents labelled with provenance ("your file", "agent-written by claude") and never
  becomes trusted memory, a decision, a lock or a guardrail by itself. Instruction-like text inside it is not followed by the daemon.
- **L0/L1 without a model:** L0 = title, then first sentence, within 100 tokens; L1 = heading outline plus the first line under each heading and any
  lines starting with Decision/Rejected/Must/TODO, within about 800 tokens. An `[llm]` profile may offer better abstracts as suggestions.
- **Search:** FTS5 over chunks (heading-split); optional vectors reuse the existing embedding profile; results are path + L0 + snippet.
- **Linking:** brief items, decisions and room tasks can point at a library path (`ref_id`), so "see plans/prd-cart.md" survives a hand-off.

## 5. Modular code
`crates/automemory-core/src/library/`: `model.rs`, `store.rs` (tables `library_files`, `library_revisions`, `library_fts`), `tiers.rs`
(deterministic L0/L1), `materialize.rs` (symlink or copy, `.git/info/exclude`, INDEX.md), `watch.rs` (reuses `docs/watcher.rs` to pick up edits
made in the project folder and in the library folder), `sync.rs` (vault ops). CLI `commands/library.rs`, MCP tools in `tools.rs`, routes
`/api/library*` with tiers (read = Safe; add/edit/delete/revert = Protected; agent `put` of its own file = Safe), page `pages/library.vue`.
Routes call only `library::` functions, like the rest of the project.

## 6. Sync
- A new delta record kind `library_file` (revision with content hash) and an encrypted blob store `library/<hash>.enc` in the vault, same key as
  everything else, so the git host sees ciphertext only. Plaintext deltas are ignored when a key exists (existing rule).
- Same project identity as memories (git remote key), so the files appear on the other computer after `sync auto`. Automatic like the brief, because
  the person chose to put the file in the library; `scope: personal` files never leave the machine.
- Conflicts (same path edited on two computers): keep both revisions, newest wins in the tree, the older one is offered as a suggestion in the Inbox
  (existing conflict handling); nothing is overwritten without a trace.
- Teammates: a file arrives labelled `from <person>` (existing `shared_by`) and only when the person opts in to project-wide library sharing.

## 7. Phases
| Phase | Scope | Done when |
|---|---|---|
| L1 | Store, revisions, secrets check, deterministic tiers, CLI add/new/ls/show/search, MCP list/get/search, hook index, rules text | An agent in a fresh session lists and reads a library file; `git status` untouched |
| L2 | Materialize (symlink/copy), `.git/info/exclude`, INDEX.md, watcher both ways, `doctor` check | A file edited in the project folder lands in the library; a test proves git stays clean |
| L3 | Vault sync as `library_file`, personal vs shared scope, conflict suggestions | A file added on computer A is readable on B after sync; ciphertext in the remote |
| L4 | Dashboard Library page (tree, tiers, editor, history, revert, git-clean banner), Docs page shows library and in-repo files together with a source badge | All actions from the page, Protected ones ask the password |
| L5 | `library.put`/`propose`, links from brief/decisions/rooms, "move out of git" helper (`library add --move`, prints the `git rm --cached` command, never commits) | Agent-made files appear labelled; moving a PRD out of the repo takes one command |
| L6 | Images/PDF as attachments, optional `[llm]` abstracts (suggestions), per-agent notes ("tell Cursor to read this") | Decide after L1-L5 are used |

## 8. Risks and answers
- **Some agents do not read git-excluded paths or do not follow symlinks.** MCP and the hook index cover agents with either; the rules text
  gives the path; a copy mode exists. Test each adapter in a manual trial (like T11) before claiming support.
- **Index bloat.** L0 lines only, capped by the existing token budget; the rest by tool call.
- **Two sources of truth** (the library copy and the project-folder copy). The library is the source; the watcher syncs edits both ways with revisions; the page shows drift.
- **Secrets in pasted notes.** Refuse on add, scan on every revision, never auto-sync a refused file.
- **Windows.** Copy mode; CI check like the update/hooks tests.
- **Scope creep into a knowledge base.** Text files for agents only; no wiki features, no web clipper.

## 9. To settle with the maintainer
1. Folder name agents see: `.automemory/library/` (proposed) or something shorter such as `.docs/`?
2. Default for new files: shared with all your computers (proposed) or this machine only until marked?
3. Teammate sharing in v1 of this feature, or only your own computers?
4. Do you want existing in-repo PRDs/plans offered for "move out of git" (never done without asking), or only new files?
5. Which agents must work on day one (Claude Code, Antigravity, Codex proposed)?
