# Punch — Complete Usage Guide
**v0.6.0** · [Back to README](README.md)

---

## Global Flags

These work with every command:

| Flag | Description | Default |
|------|-------------|---------|
| `--server <url>` | Signalling server URL | `wss://punch-8o2u.onrender.com` |
| `--log` | Log session to `~/.punch/logs/sessions.json` | off |
| `-v, --verbose` | Debug output | off |

---

## `punch generate` — Create a connection code

```bash
punch generate                   # T-No: single use
punch generate --uses 10         # Q-No: 10 uses
punch generate --permanent       # P-No: permanent, needs verify
punch generate --server ws://localhost:8080
```

| Token | What it means |
|-------|--------------|
| **T-No** | Expires immediately after one session. Nothing stored. |
| **Q-No** | Decrements per connection. Stored in `~/.punch/tokens.json`. |
| **P-No** | Permanent. Blocked until `punch verify <code>`. |

---

## `punch listen <code>` — Reconnect without burning a use

Reuses an existing Q-No or P-No token. The token use is only consumed once a peer actually connects.

```bash
punch listen 7731
punch listen 7731 --server ws://localhost:8080
```

**Home server pattern:**
```bash
# First time only
punch generate --uses 100    # → Q-No: 7731
punch verify 7731

# Every subsequent reconnect
punch listen 7731
```

---

## `punch connect <code>` — Connect to a peer

```bash
punch connect 4829
punch connect 4829 --server wss://punch-8o2u.onrender.com
punch connect 4829 --log
```

**Connection states:**
```
Punching...                           → hole punch attempt
Punched! Direct connection.           → p2p, server gone
Couldn't punch. Relaying...         → relay fallback
Connected via encrypted relay.        → E2E encrypted via relay.iroh.network
Keys exchanged. End-to-end encrypted.
```

---

## `punch send <file>` — Send a file

```bash
punch send video.mp4
punch send /path/to/archive.zip
punch send setup.exe                 # receiver sees HIGH RISK warning
```

**What happens:**
1. File checksummed (SHA256)
2. Split into dynamic chunks (1MB → 64MB based on file size)
3. T-No code generated and displayed
4. Receiver sees full consent prompt before anything transfers
5. File streams peer-to-peer over 4 parallel Iroh QUIC streams
6. SHA256 verified per chunk and whole file

**Chunk sizing:**

| File size | Chunk size |
|-----------|------------|
| < 100 MB | 1 MB |
| 100 MB – 1 GB | 4 MB |
| 1 GB – 10 GB | 16 MB |
| > 10 GB | 64 MB |

**Resume by token type:**

| Token | If transfer drops |
|-------|-----------------|
| T-No | Cannot resume — generate new code |
| Q-No (1 use left) | Cannot resume — warned before sending |
| Q-No (2+ uses) | Resume costs 1 additional use |
| P-No | Always resumable |

---

## `punch receive <code>` — Receive a file

### Save to current directory
```bash
punch receive 6241
# Check where you are first: pwd (Linux/macOS) or cd (Windows)
```

### Save to specific location
```bash
# Windows
punch receive 6241 --dest "C:\Users\DELL\Downloads"
punch receive 6241 -d "D:\Projects"

# Linux / macOS
punch receive 6241 --dest ~/Downloads
punch receive 6241 -d /tmp
```

**Consent prompt (always shown):**
```
──────────────────────────────────────────────────
  Incoming file transfer request
──────────────────────────────────────────────────
  File:        setup.exe
  Size:        42.0 MB
  Type:        .exe
  Risk:        HIGH RISK
  Info:        Executable — can run code on your system
  Checksum:    a3f912bc77de1234...
  Fingerprint: a3f9-12bc-77de
──────────────────────────────────────────────────
  Accept? (yes/no):
```

30 second timeout. No response = auto-rejected.

**Risk levels:**

| Level | Extensions |
|-------|-----------|
| HIGH | `.exe .bat .sh .ps1 .dll .app .dmg .pkg .vbs` |
| MEDIUM | `.zip .tar .gz .rar .7z .apk .jar` |
| LOW | `.pdf .jpg .mp4 .txt .docx` and most others |

**Acceptance always logged** to `~/.punch/logs/transfers.json`.

**Resume:** same command, Punch detects partial file automatically.

---

## `punch forward expose <port>` — Expose a local port

```bash
punch forward expose 8096                  # TCP, T-No token
punch forward expose 8096 --udp           # TCP + UDP
punch forward expose 8096 --uses 10       # Q-No: 10 sessions
punch forward expose 8096 --permanent     # P-No: always accessible
punch forward expose 3000 --server ws://localhost:8080
```

**What you see:**
```
T-No: 9182
Type: T-No
Protocol: TCP
Port: 8096

Starting Iroh endpoint... done
Node ID: ki6htfv...
Session fingerprint: 3a7f-12bc-88de
   Ask connector to verify this fingerprint.

Waiting for connector...

Connector connected — forwarding port 8096 (TCP)
   Session: 3a7f-12bc-88de
   Press Ctrl+C to stop.

  → TCP stream opened (1 active)
  → TCP stream opened (2 active)
  ← TCP stream closed (1 active)
```

---

## `punch forward connect <code>` — Connect to a forwarded port

```bash
punch forward connect 9182                 # auto-assign local port
punch forward connect 9182 --local 8096   # specific local port
punch forward connect 9182 --udp          # enable UDP
```

**Consent prompt:**
```
─────────────────────────────────────────
  Incoming port forward request
─────────────────────────────────────────
  Remote port:  8096 (TCP)
  Token type:   T-No
  Fingerprint:  3a7f-12bc-88de
─────────────────────────────────────────
  Verify fingerprint with exposer.
  Connect? (yes/no):
```

**After connecting:**
```
Connected (Iroh QUIC — direct or relay, automatic)
Forwarding:
   TCP: localhost:54231 → remote:8096
   Fingerprint: 3a7f-12bc-88de
   Press Ctrl+C to disconnect.
```

Open `http://localhost:54231` in your browser (or whatever port was assigned).

---

## `punch shell` — Remote terminal over Iroh QUIC

Traffic is **peer-to-peer** (same Iroh stack as file transfer and port forward). The signalling server only relays the shell **handshake** (like forward). The host machine runs a real PTY (`cmd.exe` on Windows, `$SHELL` elsewhere); the client gets an interactive terminal after the host approves.

### Host (Device B — the machine whose shell is shared)

```bash
punch shell host
punch shell host --uses 10              # Q-No token
punch shell host --permanent            # P-No (run punch verify first)
punch shell host --server ws://localhost:8080
```

1. Prints **T-No / Q-No / P-No** code — share with the client.  
2. Waits for an Iroh connection, then prompts: **Allow shell access?** / **Keep shell alive if client disconnects?**  
3. After approval, shows a live **command monitor**; **Ctrl+K** kills the session on the host.

### Client (Device A)

```bash
punch shell connect 4829
punch shell connect 4829 --server ws://localhost:8080
```

1. Loads host fingerprint from signalling; **verify** it matches the host console.  
2. Waits for host approval, then attaches to the remote shell. **Ctrl+C** exits the client.

### Notes

- **Order:** start **host** (or at least be past signalling so the client can get the handshake), then **connect** from the client.  
- **`-v` / `--verbose`:** extra tracing (e.g. stream setup) for debugging.  
- Host-side **blocklist / suspicious patterns** and session logs are configured under `~/.punch/` (see `shell_config`); session history is appended to **`~/.punch/logs/shell_sessions.json`** when the session ends.

---

## `punch verify <code>` — Activate a P-No token

```bash
punch verify 7731
# Confirm? (yes/no): yes
# Token 7731 verified.
```

---

## `punch revoke <code>` — Kill a token

```bash
punch revoke 7731
# Token 7731 revoked.
```

---

## `punch tokens` — List active tokens

```bash
punch tokens

# Output:
# Code     Type         Status                         Created
# ────────────────────────────────────────────────────────────
# 7731     Q-No         Quantised (7 uses remaining)   2026-05-01 14:22
# 9812     P-No         Permanent (verified)           2026-05-01 10:30
```

---

## `punch dashboard` — Local web UI

```bash
punch dashboard
# Opens http://localhost:7777
```

Shows session history, token status, file transfer log, port forward log (shell sessions: see `~/.punch/logs/shell_sessions.json` until dashboard integration). Reads local files only — zero external requests.

---

## Common Workflows

### Share a dev server with a teammate
```bash
# You (Device A)
punch forward expose 3000
# Share code → teammate

# Teammate (Device B)
punch forward connect <code>
# → Access http://localhost:<auto-port>
```

### Send a large file reliably
```bash
# Generate a reusable token
punch generate --uses 5
# Q-No: 4829

punch send bigfile.iso
# If it drops: run exact same command, it resumes
```

### Access Jellyfin from anywhere
```bash
# Home server
punch generate --permanent
# P-No: 7731
punch verify 7731

# Every day
punch listen 7731

# From anywhere
punch forward connect 7731 --local 8096
# Open http://localhost:8096
```

### Remote shell (support / server access)
```bash
# Machine you’re helping (host)
punch shell host --server ws://localhost:8080
# Share code + fingerprint out of band

# Your laptop (client)
punch shell connect <code> --server ws://localhost:8080
```

### Self-hosted everything
```bash
# Run your own server
cd server && go run cmd/main.go

# All commands work with --server
punch generate --server ws://localhost:8080
punch connect <code> --server ws://localhost:8080
punch send file.zip --server ws://localhost:8080
punch forward expose 8096 --server ws://localhost:8080
punch shell host --server ws://localhost:8080
punch shell connect <code> --server ws://localhost:8080
```

---

## Local Files

| Path | Contents | Always? |
|------|----------|---------|
| `~/.punch/tokens.json` | Q-No + P-No state | On create |
| `~/.punch/logs/sessions.json` | Connections | Only `--log` |
| `~/.punch/logs/transfers.json` | Accept/reject decisions | **Always** |
| `~/.punch/logs/forward.json` | Port forward sessions | On forward |
| `~/.punch/logs/shell_sessions.json` | Shell session log (host) | When shell session ends |
| `<dest>/<file>.punch_partial` | In-progress file | During transfer |
| `<dest>/<file>.punch_state` | Chunk state for resume | During transfer |