# Punch — Usage Guide

> Complete reference for all commands, flags, and workflows.

---

## Installation

### Download prebuilt binary (recommended)
Go to [GitHub Releases](https://github.com/MannanSaood/Punch/releases) and download the binary for your platform:

| Platform | File |
|----------|------|
| Windows (x64) | `punch-windows-x86_64.exe` |
| Linux (x64) | `punch-linux-x86_64` |
| macOS (Intel) | `punch-macos-x86_64` |
| macOS (Apple Silicon) | `punch-macos-arm64` |

### Build from source
```bash
git clone https://github.com/MannanSaood/Punch.git
cd Punch/core
cargo build --release
# Binary at: target/release/punch
```

---

## Global Flags

These flags work with every command:

| Flag | Description | Default |
|------|-------------|---------|
| `--server <url>` | Signalling server URL | `wss://punch-8o2u.onrender.com` |
| `--log` | Enable local session logging to `~/.punch/logs/sessions.json` | off |
| `-v, --verbose` | Show debug output | off |

---

## Commands

### `punch generate` — Create a connection code

Generates a code and waits for a peer to connect.

```bash
punch generate
punch generate --uses 5
punch generate --permanent
punch generate --server ws://localhost:8080
punch generate --log
```

**Flags:**

| Flag | Description |
|------|-------------|
| `--uses <N>` | Q-No mode — expires after N connections |
| `--permanent` | P-No mode — persistent, requires verification |

**Token types:**

| Token | Command | Behaviour |
|-------|---------|-----------|
| **T-No** | `punch generate` | Single session. Expires immediately after. Nothing stored. |
| **Q-No** | `punch generate --uses 5` | Expires after N uses. Stored in `~/.punch/tokens.json`. |
| **P-No** | `punch generate --permanent` | Permanent. Requires `punch verify <code>` before first use. |

**Example output:**
```
T-No: 4829
Type: T-No
Waiting for peer...
```

---

### `punch listen <code>` — Reconnect on existing token

Waits for a peer on an **existing** stored token without generating a new one.
Use this to reconnect after a session ends without consuming an extra Q-No use.

```bash
punch listen 4829
punch listen 4829 --server ws://localhost:8080
punch listen 4829 --log
```

**When to use listen vs generate:**

| Situation | Command |
|-----------|---------|
| First connection ever | `punch generate --uses 10` |
| Reconnecting after session ends | `punch listen <code>` |
| New one-time connection | `punch generate` |

**Example — home server workflow:**
```bash
# First time only
punch generate --uses 20
# → Q-No: 7731

# Every reconnect after that
punch listen 7731
```

---

### `punch connect <code>` — Connect to a peer

Connects to a device that is waiting with `punch generate` or `punch listen`.

```bash
punch connect 4829
punch connect 4829 --server wss://punch-8o2u.onrender.com
punch connect 4829 --log
```

**Example output:**
```
Connecting with code: 4829

Punching...
✅ Punched! Direct connection established.

Session active. Press Ctrl+C to disconnect.
```

or if hole punch fails:
```
❌ Couldn't punch. Relaying encrypted traffic.
🔒 Connected via encrypted relay.
🔑 Keys exchanged. End-to-end encrypted.

Session active via encrypted relay. Press Ctrl+C to disconnect.
```

---

### `punch send <file>` — Send a file

Sends a file directly to a peer. Uses IDM-style parallel chunked transfer —
file goes directly peer to peer, never through the server.

```bash
punch send video.mp4
punch send /path/to/file.zip
punch send document.pdf --server ws://localhost:8080
punch send setup.exe --log
```

**What happens:**
1. File is split into chunks (size depends on file size — see below)
2. SHA256 checksum computed for whole file and each chunk
3. A T-No code is generated and displayed
4. You share the code with the receiver
5. Receiver connects and sees a consent prompt before any data flows
6. File streams directly peer to peer over 4 parallel TCP connections
7. Receiver verifies checksum — transfer complete

**Dynamic chunk sizing:**

| File size | Chunk size | Approx chunks |
|-----------|------------|---------------|
| < 100 MB | 1 MB | ~100 |
| 100 MB – 1 GB | 4 MB | ~250 |
| 1 GB – 10 GB | 16 MB | ~625 |
| > 10 GB | 64 MB | ~160 |

**Resume behaviour by token type:**

| Token | Resume? | Notes |
|-------|---------|-------|
| T-No | ❌ No | Single use — if it drops, generate a new code |
| Q-No (1 use left) | ❌ No | Last use — warned before sending |
| Q-No (2+ uses) | ✅ Yes | Resume costs 1 additional use |
| P-No | ✅ Yes | Unlimited reconnects, always resumable |

**Example output (sender):**
```
📁 File: video.mp4 (1,240 MB)
🔐 Computing checksum... done
📦 310 chunks × 4MB (dynamic sizing)
📡 Listening on port 49201

──────────────────────────────────────────
  📤 Sending file
──────────────────────────────────────────
  File:        video.mp4
  Size:        1240.0 MB
  Risk level:  🟢 LOW RISK
  Fingerprint: a3f9-12bc-77de
──────────────────────────────────────────
  Share fingerprint with receiver for verification.

T-No: 6241
Share this code with the receiver.

Waiting for receiver...
```

---

### `punch receive <code>` — Receive a file

Receives a file from a peer sending with `punch send`.

#### Save to current directory (where command is running)
```bash
punch receive 6241
```

File saves to whichever directory your terminal is currently in.
Check with `pwd` (Linux/macOS) or `cd` (Windows) before running.

#### Save to a specific location
```bash
# Windows
punch receive 6241 --dest "C:\Users\DELL\Downloads"
punch receive 6241 --dest "D:\Projects\files"
punch receive 6241 -d "C:\Users\DELL\Desktop"

# Linux / macOS
punch receive 6241 --dest ~/Downloads
punch receive 6241 --dest /home/user/files
punch receive 6241 -d /tmp
```

**Flags:**

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--dest <path>` | `-d` | Directory to save the file into | `.` (current directory) |

**Consent prompt — always shown before accepting:**
```
──────────────────────────────────────────────────
  📦 Incoming file transfer request
──────────────────────────────────────────────────
  File:        video.mp4
  Size:        1240.0 MB
  Type:        .mp4
  Risk:        🟢 LOW RISK
  Info:        Document or media — generally safe
  Checksum:    a3f912bc77de1234...
  Fingerprint: a3f9-12bc-77de
──────────────────────────────────────────────────

  Tip: Ask the sender to confirm fingerprint: a3f9-12bc-77de

  Accept? (yes/no):
```

You have **30 seconds** to accept. No response = auto rejected.

**Risk levels:**

| Level | Extensions | Warning |
|-------|-----------|---------|
| 🔴 HIGH RISK | `.exe .bat .sh .ps1 .dll .dmg .pkg` and more | Strong warning shown |
| 🟡 MEDIUM RISK | `.zip .tar .gz .rar .7z .apk` and more | Note to scan with antivirus |
| 🟢 LOW RISK | `.pdf .jpg .mp4 .txt .docx` and more | No extra warning |

**Acceptance is always logged** to `~/.punch/logs/transfers.json` regardless of `--log` flag.

**Resume:** if connection drops mid-transfer, run the exact same command again.
Punch detects the `.punch_partial` file and resumes from where it stopped.

---

### `punch verify <code>` — Verify a P-No token

Must be run before a P-No permanent token can be used for the first time.

```bash
punch verify 7731
```

**Output:**
```
Verifying permanent token: 7731
This will allow permanent access to your device.
Confirm? (yes/no): yes
✅ Token 7731 verified. Permanent access enabled.
```

---

### `punch revoke <code>` — Revoke a token

Immediately invalidates a token. Any future connection attempt with this code is rejected.

```bash
punch revoke 7731
```

**Output:**
```
🗑️  Token 7731 revoked.
```

---

### `punch tokens` — List active tokens

Shows all stored Q-No and P-No tokens with their current status.

```bash
punch tokens
```

**Output:**
```
Active tokens:

Code     Type         Status                         Created
────────────────────────────────────────────────────────────────────────
7731     Q-No         Quantised (3 uses remaining)   2026-04-29 17:00
9812     P-No         Permanent (verified)           2026-04-29 16:30
4401     P-No         Permanent (not verified)       2026-04-29 16:00
```

---

### `punch dashboard` — Local session dashboard

Opens a local web dashboard showing session history and token activity.
Zero external requests — reads only from `~/.punch/logs/`.

```bash
punch dashboard
# Opens http://localhost:7777 in your browser
```

---

## Common Workflows

### Share a file with a friend (one time)
```bash
# You
punch send photo.jpg

# Friend (replace 1234 with your code)
punch receive 1234 --dest ~/Downloads
```

### Share a large file reliably (resumable)
```bash
# You — generate a Q-No token with enough uses
punch generate --uses 5

# Then send
punch send bigfile.zip

# Friend
punch receive 1234 --dest D:\Downloads
```

### Access your home server repeatedly
```bash
# Home server — first time only
punch generate --uses 50
# → Q-No: 7731

punch verify 7731

# Home server — every subsequent time
punch listen 7731

# Your laptop — anywhere in the world
punch connect 7731
```

### Self-hosted server
```bash
# Run your own signalling server
cd server && go run cmd/main.go

# Point CLI at it
punch generate --server ws://localhost:8080
punch connect 1234 --server ws://localhost:8080
punch send file.zip --server ws://localhost:8080
punch receive 1234 --dest ~/Downloads --server ws://localhost:8080
```

---

## Local Data

Punch stores everything locally. Nothing leaves your device except connection handshake metadata.

| Path | Contents |
|------|----------|
| `~/.punch/tokens.json` | Q-No and P-No token state |
| `~/.punch/logs/sessions.json` | Session history (only with `--log`) |
| `~/.punch/logs/transfers.json` | Transfer acceptance log (always written) |
| `<dest>/<filename>.punch_partial` | Partial file during transfer |
| `<dest>/<filename>.punch_state` | Chunk state for resumption |

---

## Network Notes

```
Punch works best on WiFi.
Mobile (4G/5G) and corporate networks may fall back to relay.
```

- **Direct connection** — hole punching succeeds, file goes peer to peer
- **Relay fallback** — encrypted end-to-end, server sees nothing
- **File transfer** — always direct TCP peer to peer, never through server
- **STUN failure** — automatically falls back to relay, no action needed

---

*Punch v0.4.0 — [github.com/MannanSaood/Punch](https://github.com/MannanSaood/Punch)*