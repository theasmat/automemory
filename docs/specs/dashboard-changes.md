# Dashboard: file changes from Tracewise (Nov 2026)

Status: built. Decision: show Tracewise's record of file changes inside the AutoMemory dashboard as a thin, read-only view that joins on the session, and leave everything that edits files (undo, restore, diffs, blame) to Tracewise.

## Why a thin view

Tracewise already has a desktop app with a timeline, line-by-line history and undo. A second copy of that would add maintenance and a second set of buttons that rewrite files. What neither product does alone is the join: Tracewise's `actor.session` is the same id AutoMemory knows, so "this session edited these files, saved these notes and recorded this decision" needs both.

## What is built

| Where | What |
|---|---|
| Agents & Locks | A "Recent file changes" card under the locks (time, agent, operation, file, the reason the agent gave, and a mark when the edit was made while another agent held the file). Refreshes every 10 seconds while the page is open |
| Activity, Sessions | A replayed session shows "Files this session changed": Tracewise's changes with the same session id |
| Note and decision dialog | "Files this note is about": for a note that names files (`metadata.files_modified` or `files`), the latest Tracewise changes to them |
| `automemory stats` and the Home card | "Edit collisions prevented" counts the refusals Tracewise made (the hook records `source: tracewise` on the audit row); guardrail blocks stay under "risky commands" |

## How it works

`GET /api/changes?project_id=&session=&paths=a,b&limit=` asks `tracewise -C <project folder> log --json` (for a file, `log <path>`), reads each record into a small struct (`companion::FileChange`) and answers `{source: "tracewise" | "none", changes, note?}`. Answers are cached for five seconds per query, so each page costs at most one process start per refresh. A session filter needs at least eight characters of the id and reads up to 500 records back. At most five files are looked up for a note.

Without Tracewise (or off macOS) the answer is an empty list with a sentence, and the cards say where to install it.

## Rules

- Read-only. Nothing here changes a file; the cards point to the Tracewise app and `tracewise log` for diffs and undo.
- The reason text is written by agents: it is shown as plain text, with the agent's name, and is never treated as an instruction.
- A change list never holds data of its own: it is Tracewise's, fetched each time.

## Not built

A full timeline, diffs, blame and undo in AutoMemory (Tracewise's app has them with a preview before any undo); a deep link into the Tracewise app (it has no URL scheme yet).
