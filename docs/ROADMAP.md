# Punch — Roadmap

> Philosophy: Ship something real at every milestone. No milestone ends in "in progress."

---

## v0.1 ✅ — Core Connection
**Shipped. Two devices. One code. Direct connection.**

- WebSocket signalling server (Go)
- UDP hole punching via STUN
- Encrypted relay fallback (automatic)
- T-No ephemeral tokens
- Connection state messages
- Startup network advisory
- `--server` flag for self-hosted instances
- Single binary + Dockerfile

---

## v0.2 ✅ — Encrypted Relay
**Shipped. Zero knowledge claim is now actually true.**

- X25519 Diffie-Hellman key exchange
- ChaCha20-Poly1305 end-to-end encryption
- Server sees two public keys, cannot derive shared secret
- Key exchange happens transparently after matching
- `🔑 Keys exchanged. End-to-end encrypted.` confirmation

---

## v0.3 ✅ — Token Enforcement
**Shipped. T-No, Q-No, P-No all enforced.**

- Token state persisted to `~/.punch/tokens.json`
- T-No — ephemeral, nothing stored, single use
- Q-No — usage counter persists across restarts
- P-No — requires `punch verify` before first use
- `punch listen <code>` — reconnect without burning a token use
- `punch tokens` — list all active tokens
- `punch revoke <code>` — immediate invalidation
- Enforcement on generator side (Device A owns policy)

---

## v0.4 ✅ — File Transfer
**Shipped. Direct peer to peer, resumable, safe.**

- IDM-style parallel chunked transfer (4 streams)
- Dynamic chunk sizing (1MB / 4MB / 16MB / 64MB based on file size)
- Direct TCP peer to peer — server never sees file data
- SHA256 verification per chunk and whole file
- Resumable — `.punch_partial` + `.punch_state` survive restarts
- Idempotent chunk requests — ACK-lost scenario handled
- Connection drop vs data corruption distinguished correctly
- State saved every 512KB — minimal data lost on drop
- Risk classification — 🔴 HIGH / 🟡 MEDIUM / 🟢 LOW
- Consent prompt with 30 second timeout before any data flows
- Session fingerprint for verbal verification
- Acceptance always logged to `~/.punch/logs/transfers.json`
- Resume warnings by token type (T-No, Q-No last use, P-No)
- `punch send <file>` and `punch receive <code> --dest <path>`

---

## v0.5 — Port Forwarding
**Goal: Expose a local port through Punch to a remote device.**

```bash
# Device A — expose local port 3000
punch forward 3000

# Device B — access it
punch forward connect <code>
# → localhost:3000 now accessible on Device B
```

- Direct TCP tunnel between ports
- No server involvement after handshake
- Consent prompt on receiving side
- Works for any TCP service — HTTP, SSH, databases, game servers
- Clean disconnect when either side exits

**Done when:**
> Running a local dev server on port 3000, a friend on a different network can access it via `punch forward connect <code>` in under 10 seconds.

---

## v0.6 — Remote Terminal
**Goal: Secure shell access with explicit consent and monitoring.**

```bash
# Device A — allow terminal access
punch shell

# Device B — request terminal
punch shell connect <code>
```

- Explicit consent prompt on host side before shell opens
- Session-scoped — access ends when connection ends
- P-No tokens for persistent home server access
- Real-time command visibility on host side
- eBPF monitoring — detect access to sensitive paths
- Auto-kill on suspicious activity (configurable)
- Every command logged locally on host

**Done when:**
> Full shell session works across different networks. Host can see and kill session at any time.

---

## v0.7 — Local Dashboard
**Goal: Visualise everything Punch has done, locally.**

```bash
punch dashboard
# → http://localhost:7777
```

- Session history — connection type, duration, data transferred
- Token activity — Q-No remaining uses, P-No last used
- Transfer history — files sent/received, risk levels, decisions
- Terminal session log — commands run per session (v0.6 data)
- Zero external requests — reads local files only
- Auto-refreshes every 10 seconds
- Built in Svelte, compiled static bundle

**Done when:**
> `punch dashboard` shows complete history with no internet connection.

---

## v0.8 — Developer Library
**Goal: Other developers can embed Punch in their apps.**

```rust
// Cargo.toml
punch-core = "0.8"

// Usage
let conn = punch::connect("4829").await?;
conn.send(data).await?;
let received = conn.recv().await?;
```

- Extract core into `punch-core` crate
- Clean public API: `connect()`, `send()`, `recv()`, `close()`
- Async/Tokio throughout
- Published to crates.io
- Full docs on docs.rs
- Integration test suite

**Done when:**
> Someone outside the project builds a working p2p app using `punch-core` from crates.io with under 20 lines of code.

---

## v1.0 — Public Launch
**Goal: Production ready. Real world tested. Documented.**

### Hardening
- [ ] Security audit of relay encryption
- [ ] Fuzz testing on STUN and protocol parsing
- [ ] Load test signalling server (1000 concurrent sessions)
- [ ] Symmetric NAT comprehensive testing
- [ ] Windows full test suite
- [ ] ARM builds (Raspberry Pi)

### Distribution
- [ ] `cargo install punch-cli`
- [ ] Homebrew formula
- [ ] Prebuilt binaries via GitHub Releases (all platforms)
- [ ] Docker Hub image for server

### Documentation
- [ ] Full docs site (GitHub Pages)
- [ ] Self-hosting guide
- [ ] Protocol specification (RFC-style)
- [ ] Security model writeup

**Done when:**
> Someone who has never heard of Punch can install it, connect two devices, and understand what happened — in under 5 minutes.

---

## Post v1.0 — Future Ideas

Not committed. Only after v1.0 ships.

- **QUIC transport** — replace TCP in file transfer for better performance on lossy networks
- **Multi-peer sessions** — connect more than two devices
- **Mobile CLI** — Termux support for Android
- **Prometheus metrics** — for self-hosters who want server observability
- **punch sync** — folder sync between two devices

---

## Milestone Summary

| Version | Focus | Key Deliverable | Status |
|---------|-------|-----------------|--------|
| v0.1 | Core | Direct p2p connection | ✅ Shipped |
| v0.2 | Security | Relay encrypted, zero knowledge true | ✅ Shipped |
| v0.3 | Access Control | Token enforcement, listen command | ✅ Shipped |
| v0.4 | File Transfer | IDM chunked, resumable, consent | ✅ Shipped |
| v0.5 | Port Forwarding | punch forward | 🔜 Next |
| v0.6 | Terminal | punch shell + monitoring | 🔜 |
| v0.7 | Dashboard | Local visualisation | 🔜 |
| v0.8 | Ecosystem | Developer library on crates.io | 🔜 |
| v1.0 | Launch | Production ready, publicly announced | 🔜 |