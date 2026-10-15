# Autonomous Guardrails & Catastrophe Interception

AutoMemory features an active AST tokenizer and command safety evaluator that validates planned agent operations before execution.

---

## 🛡️ Autopilot Operating Modes

Configure safety behavior per project using `automemory mode set <mode>`:

### 1. `Vibe` Mode (Default — Maximum Velocity)
- **Philosophy**: Non-intrusive assistance. Never interrupt the coding flow for benign or standard development tasks.
- **Behavior**:
  - High-risk operations (e.g., standard database migrations, git clean) emit inline warning notifications without pausing execution.
  - Critical catastrophic actions (`rm -rf /`, raw disk formatting, plain-text secret leakage) are strictly blocked.

### 2. `Strict` Mode (High Safety)
- **Philosophy**: Defensive verification.
- **Behavior**:
  - All critical and high-risk operations are halted and require explicit operator confirmation before tool execution can proceed.

### 3. `Review` Mode (Human-in-the-Loop Governance)
- **Philosophy**: Multi-agent review and audited changes.
- **Behavior**:
  - High-risk operations are automatically intercepted, packaged into a `ReviewProposal`, and held in the pending queue.
  - Commands can only proceed once approved via the Web Dashboard or `memory.proposals.decide`.

---

## 🚨 Built-in Safety Policies

AutoMemory parses commands using structural tokenizers rather than simple string grep to prevent evasion (such as subshell wrapping, variable interpolation, or whitespace manipulation).

### Critical Catastrophic Rules (Blocked across ALL modes):
| Rule Name | Target Risk | Example Intercepted Commands |
|---|---|---|
| **Root/Home Deletion** | Recursive deletion of `/`, `~`, or `$HOME` | `rm -rf /`, `rm -rf /*`, `sudo rm -rf ~` |
| **Disk Format / Raw Write** | Direct write to physical block devices | `mkfs.ext4 /dev/sda1`, `dd if=/dev/zero of=/dev/rdisk` |
| **Protected Branch Force-Push** | Overwriting canonical production branches | `git push origin main --force`, `git push -f prod` |
| **Database Drop / Truncate** | Complete loss of database tables | `DROP DATABASE production`, `TRUNCATE TABLE users` |
| **Plaintext Secret Leakage** | Hardcoding API keys in terminal commands | `OPENAI_API_KEY=sk-proj...`, `ghp_...` |

### High-Risk Rules (Halted in `Strict` & Queued in `Review`):
| Rule Name | Target Risk | Example Intercepted Commands |
|---|---|---|
| **Production Migrations** | Applying database schema shifts | `npx prisma migrate reset`, `alembic upgrade head` |
| **Destructive Git Reset** | Discarding uncommitted workspace changes | `git reset --hard HEAD~1`, `git clean -fd` |
| **Infrastructure Teardown** | Terminating cloud clusters and containers | `terraform destroy`, `kubectl delete ns production` |
| **Secret File Modification** | Modifying sensitive local environment vaults | `cat > .env.production`, `rm id_rsa` |
| **Internal Database Tampering** | Raw SQLite access to AutoMemory's DB | `sqlite3 ~/.automemory/automemory.db "DELETE FROM..."` |

---

## ⚙️ Managing Custom Guardrail Rules

You can add custom domain-specific rules using the CLI:

```bash
# Add a custom rule blocking access to sensitive company keys:
automemory rule add \
  --name "Protect AWS Production Credentials" \
  --pattern "(?i)/aws/credentials" \
  --critical \
  --description "Prohibits agents from reading or modifying production AWS keys"

# List all active rules:
automemory rule list

# Toggle or disable a rule:
automemory rule toggle <rule-id> --disable

# Delete a rule:
automemory rule remove <rule-id>
```

### 🔐 Human authorization for safety writes

Rule mutations (`add`/`remove`/`toggle`), hook pipeline changes, `mode set`, and the dashboard's rule/mode endpoints require a **human password** — agents with shell or localhost access cannot satisfy it:

- Set it once: `automemory rule passwd` (or `--from-env` with `AUTOMEMORY_PASSWORD`).
- The CLI accepts the password only from `AUTOMEMORY_PASSWORD` or an interactive `/dev/tty` prompt (echo disabled). Piped stdin — how agents invoke hooks — always fails closed.
- The dashboard accepts it as the `X-AutoMemory-Password` header (403 otherwise). Read paths stay open.
- Hashes are PBKDF2-HMAC-SHA256 (600k rounds); plaintext is never stored. Until a password is set, writes are allowed once with a warning.
