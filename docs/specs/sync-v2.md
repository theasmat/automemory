# Sync v2: who may write, and how to cut a computer off

Status: built (Nov 2026). Builds on the op-log in [sync](../architecture/sync.md); the choices are in [decisions](../decisions.md). Evidence behind the design: the Nov 2026 architecture research (signed per-device events were an inference there, not a tested result).

## What did not change

Delta files, content fingerprints, the base-fingerprint three-way merge (no clocks), conflict review items, `project_key`, encrypted `.enc` files, the git remote. A record changed on two computers still becomes a review item and the local copy stays. Locked preferences stay on their machine (they are not a synced record kind).

## What it fixes

| Gap | Fix |
|---|---|
| An agent could edit or archive a note a person wrote (MCP `memory.update` / `archive` / `restore` had no check), and a synced copy that was not person-authored could fast-forward over one | `Memory::is_person_authored` (tier 1 or 3). The MCP tools refuse; sync files a `sync_authority` review item and keeps the person's note |
| Two computers each recorded a decision for one topic; both stayed in force silently | One `sync_conflict` review item (`decision-topic:…`); both are kept until a person records the one they want |
| Anyone with the vault key could claim any device or author | Each device signs its ops (ed25519). A device announces its public key in a `device` record; an op from an unknown device, with a bad signature, or from a revoked device is refused (`rejected` in the report) |
| A lost computer could not be cut off | `sync devices revoke <id>` rotates the vault key (below) |
| "Forget" left the secret in old versions, other computers and git history | `memory scrub` + `sync purge-history` |
| The vault only grew | `sync purge-history` compacts to one snapshot |

## Trust model

The root of trust is the **recovery phrase**: whoever holds the vault key can enrol a computer (first announcement for a device id wins; a second, different key for the same id is refused). This is deliberate: pairing stays one phrase. A signature proves *which device* wrote an op, not that the device is honest. The signing seed sits in `sync_kv`, protected like the database file (which already holds every note in plain text); revoke a computer whose file was copied.

Wire format: ops carry `v: 2`, `seq`, `sig` (and `scrub`). The signature covers kind, id, fingerprint, device, seq, author, time, project key and the scrub flag. Unsigned (v1) ops from a device that never announced are accepted until `sync devices strict on`; an op without a signature from a device that has announced is always refused. Revoking turns strict mode on everywhere.

## Revoking a computer

`automemory sync devices revoke <id>` (human password): a new vault key is made; the revoking computer writes `epochs/epoch-N.json`, signed, holding the new key wrapped to every other active computer (X25519 + AES-256-GCM) and the older keys encrypted under the new one. Other computers read the bulletin on their next sync, adopt the key (and write it to the keychain) and mark the device revoked. The revoked computer finds the bulletin, stops syncing, and cannot read or write anything new. The old recovery phrase no longer opens new data: a computer paired later needs the **new** phrase, which the command prints. Files written before the revoke stay readable (the old keys are kept). Ops the revoked computer writes afterwards, even under the old key, are refused.

Limits: it cannot take back what the computer already read or copied. The bulletin is plain JSON (device ids and wrapped keys only). A computer that was offline follows the bulletin when it next syncs.

## Scrubbing a secret

`automemory memory scrub <id> [--text <exact>]`: removes patterns (keys, tokens) and the exact text from the memory, every earlier version, and its embedding; marks it so the next push carries `scrub: true`. Other computers apply the clean copy over theirs even if they edited it, and drop that memory's version history (they do not know the secret, so they cannot clean it by text). Writes to the database also redact patterns (memories, decisions, failures, traces).

Old copies stay in the vault's git history. `automemory sync purge-history --yes` (human password): pulls and applies everything, replaces all delta files with one signed snapshot plus the device announcements, then replaces the git history with one commit and force-pushes it. Other computers follow on their next sync (git rebases onto the new history; if it cannot, they reset to the remote and send everything again, which is harmless). Not covered: clones that already exist, the git host's own storage until it collects garbage, and a vault shared by several *people* (the snapshot is written as the person running it).

## Tests

`sync/deltas.rs`: authority gate and its allowed case, decision conflict, signed and forged ops, unknown device, v1 compat window and strict mode, key hijack, three-device revoke (rotation, adoption, cut-off, refused writes), scrub across two computers. `sync/auto.rs`: purge leaves one commit and one file, and the other computer follows. `automemory-mcp`: an agent cannot change, archive or restore a person's note. The fallback reset after a purge when git cannot rebase is not covered by a test.

## Not built

A per-person signing identity for team vaults; automatic acknowledgements before compaction (compaction needs none because a snapshot merges like any change); a ready-made "this computer was revoked" notice in the dashboard.
