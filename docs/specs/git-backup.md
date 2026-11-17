# Git backup: an encrypted copy on any git server, kept current by AutoMemory

Status: built (Nov 2026). Cloudflare as a second transport comes later (see the end).

## The decision

The backup is the vault AutoMemory already has: encrypted delta files in a git repository. The repository can be on **GitHub, GitLab, Bitbucket, Gitea, a server of your own, or a folder**; AutoMemory only needs a git address it can push to. Nothing is stored anywhere else, and the server only ever sees ciphertext.

What was missing was everything around it: a guided setup, a visible status, a proof that the copy restores, and a way to restore on a new computer without knowing the internals. This adds those. The transport (push and pull of the vault) is unchanged, so signed ops, devices, scrub and purge-history keep working.

## For the person

| Task | How |
|---|---|
| Set up | Dashboard, Sync, "Git backup", or `automemory backup git setup [address]`. Make an empty private repository first (the card and the command say how for each provider) |
| See how it is doing | The same card; `automemory backup git status [--json]`; `automemory doctor`; a line on Home when it is failing or a week old |
| Back up now | "Back up now" or `automemory backup git now` (it also runs by itself after agents work) |
| Prove it restores | "Check that it restores" or `automemory backup git verify` |
| Restore on a new computer | `automemory backup git restore <address>` (asks for the recovery phrase, hidden) |
| Turn it off | "Turn off" (password); the vault stays on this computer, the remote copy is left alone |

After setup there is nothing to do: hooks start a background sync after agent activity, and each pass records when it worked or why not.

## What setup does

1. Reads the address (`parse_remote`): https, http, ssh, `git@host:owner/repo`, or a folder. Refuses git's command-running forms (`ext::`, `fd::`), whitespace and shell characters.
2. Warns when the address carries a password or token (it would sit in plain text in the config) and never shows the token on screen.
3. Probes the repository with the person's own git sign-in (`git ls-remote`): unreachable gets a sentence with the likely fix (ssh key, `gh auth login` or `glab auth login`, wrong address, network). For https it asks once more with no credentials: if that works the repository is **public**, which is refused unless the person confirms. A repository that already has branches is refused unless confirmed (it may be a vault from another computer: use restore).
4. Points the vault at it, creates the vault key, and shows the **24-word recovery phrase once**. The terminal asks the person to type one word back; the dashboard asks to tick "I saved the recovery phrase".
5. Runs the first backup and **verifies it**: clones the remote into a scratch folder, replays it with the key into a scratch database and counts the notes and decisions.

## Status

Every sync pass with a remote records its outcome. `state` is `off`, `never` (set up, no success yet), `ok`, `failing` (the last attempt failed; the reason is kept), or `stale` (no success for seven days). `last_verified` is when a restore check last passed. The remote address is shown with any token hidden.

## What the backup carries, and what it cannot

The vault carries notes, decisions, hand-offs, traces, failures and skills, each tagged with the **project key** of its project so another computer can attach it to the right project. A key is the repository's remote address or root commit; a folder that is not a git repository, and a second registration of the same repository, get `local:<project id>`, which the other computer turns back into a project.

Some data has no project to attach to, and the backup says so instead of claiming "working":

| Gap | Why | Fix |
|---|---|---|
| `unknown_project` | rows filed under an id that is no registered project (an old `--project auto`) | `automemory project adopt <id> <project>` |
| `unregistered_folder` | notes written by an agent in a folder that is no project (`folder-<hash>`) | register the folder, then `project adopt` |
| `no_key` | a registered project whose folder is not on this computer, so it cannot be matched to a repository | register the folder again (`project register --path`) or remove the project (`project remove`) |

The status then reads `partial`, and the dashboard card, Home, `doctor` and `automemory backup git status` list each gap with its counts and fix. Sample data is never counted. Not carried by design: the event log (a local record of tool calls; `automemory backup` keeps a full local copy of the database, on the same disk).

An ssh remote that uses a name from `~/.ssh/config` (`git@github-work:...`) is recognised through `ssh -G`, so the provider shows correctly.

## Restore

`restore` first proves the phrase opens the backup (a clone and a replay with the key derived from the phrase) and only then stores the key and syncs. A wrong phrase changes nothing on the computer.

## Safety rules

- The recovery phrase is shown to a person only: setup and turn-off need the password in the dashboard, and the terminal flow needs a terminal or `--yes`.
- ssh never waits for a prompt in the background (`BatchMode`), so a missing key fails with a message instead of hanging a hook.
- The dashboard cannot restore (it replaces data); it shows the command.

## Tested

`sync/backup_git.rs` unit tests (addresses by provider and the refused ones, token hiding, error sentences, run recording, a clone-and-replay proof with a wrong key); scenario `git_backup.py` (two computers and a folder remote: setup, 24-word phrase, encrypted-only files, restore on the second, wrong phrase refused and harmless, remote removed then back, the dashboard API with and without the password); the browser smoke test (the wizard end to end with the password).

Not tested here: a real GitHub, GitLab or Bitbucket account. The folder remote exercises the same git commands, and the checklist in `tests/manual/LIVE_CHECKLIST.md` (section G) lists what to try on each provider.

## Later: Cloudflare

A managed service would be a second transport for the same encrypted ops, not a new data model; it comes after this works for real users. It needs the checklist in the Nov 2026 discussion: remove the dev secrets and auth bypass from `workers/sync-gateway`, real auth, quotas, server-side signature checks against the device list, account export and delete, backups of its own storage, terms and a privacy policy, and a `sync remote cloud` pairing flow.
