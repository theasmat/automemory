# Zero-Knowledge Cloud Sync & OpenViking Vault

AutoMemory solves the fundamental challenge of preserving AI agent plans, roadmaps, and cross-session knowledge across multiple machines without polluting public repositories or leaking proprietary secrets.

---

## 1. OpenViking Git-Ignored Plan Synchronization

When working with coding agents, valuable architectural plans and scratchpads (such as `plans/`, `docs/specs/`, or `.gemini/antigravity/brain/`) are often added to `.gitignore` to prevent cluttering git history. 

AutoMemory links these git-ignored files to a **Private GitHub Vault**:

```text
Local Workstation A                        Private GitHub Vault
┌───────────────────────────┐             ┌─────────────────────────┐
│ /projects/my-app          │             │ my-agent-vault (private)│
│  ├── src/ (git tracked)   │             │  └── projects/my-app/   │
│  └── plans/ (git-ignored) ├─sync push──►│      ├── plans/         │
└───────────────────────────┘             │      └── specs/         │
                                          └────────────┬────────────┘
Local Workstation B                                    │
┌───────────────────────────┐                          │
│ /projects/my-app          │◄───────sync pull─────────┘
│  └── plans/ (restored!)   │
└───────────────────────────┘
```

### Configuration (`~/.automemory/config.toml`):
```toml
[vault]
remote_url = "git@github.com:yourname/my-private-vault.git"

[[projects]]
name = "my-app"
local_dir = "/Users/username/Dev/my-app"
trackable_files = [
    "plans/**",
    "docs/specs/**",
    "ARCHITECTURE.md"
]
```

### Commands:
```bash
# Push tracked git-ignored plans to your vault:
automemory sync push

# Pull and restore plans onto a new workstation:
automemory sync pull

# Run continuous background sync watcher:
automemory sync watch --interval 10
```

---

## 2. Zero-Knowledge Blind Cloud Architecture

In addition to plan synchronization, AutoMemory synchronizes structured memories and skills across workstations through an encrypted edge gateway:

1. **Client-Side Key Derivation**: Master Encryption Keys are derived locally using **PBKDF2 with 600,000 rounds** of HMAC-SHA256. Decryption keys are never transmitted to any cloud or edge server.
2. **Authenticated Ciphertext (AES-256-GCM)**: All memory contents and skill instructions are encrypted with fresh random 96-bit nonces.
3. **Blind HMAC Hashing**: Project and memory IDs are hashed via `HMAC-SHA256(MasterKey, Salt || ID)`. Edge gateways index and query only pseudorandom 64-character hex digests.
4. **Blind Semantic Vector Search (Vectorize)**: 384-dimensional dense vector embeddings are generated on-device, queried against Cloudflare Vectorize anonymously, and the resulting encrypted payloads are decrypted locally.

---

## 3. Multi-Device BIP-39 Pairing

Pairing a secondary workstation (e.g., your laptop) takes seconds without needing to manually copy keys:

### On your primary machine:
```bash
automemory sync pair
```
This generates:
- A standard 12-word or 24-word BIP-39 mnemonic recovery phrase.
- A terminal-rendered ASCII QR code for optical pairing.

### On your new machine:
```bash
automemory sync pair --join "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
```

The master key is securely imported into your native operating system keychain:
- **macOS**: Apple Keychain via `/usr/bin/security`.
- **Linux**: Secret Service daemon or POSIX `0600` encrypted store.
- **Windows**: Windows Data Protection API (DPAPI) or encrypted credential store.
