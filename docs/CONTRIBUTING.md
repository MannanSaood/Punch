# Contributing to Punch

Thanks for wanting to contribute. Punch is open source and zero-profit — contributions keep it alive.

---

## Before You Start

Read the [README](../README.md), [USAGE guide](../USAGE.md), and [ROADMAP](ROADMAP.md) first.
Understand what Punch is before adding to it.

**The core philosophy:** simple, zero-knowledge, ephemeral.
If a contribution makes Punch more complex without a strong reason, it probably doesn't belong in v1.

---

## Setting Up

```bash
git clone https://github.com/MannanSaood/Punch.git
cd Punch

# Server (Go)
cd server && go mod download

# Core CLI (Rust)
cd core && cargo build
```

## Running Locally

```bash
# Terminal 1 — start the signalling server
cd server && go run cmd/main.go

# Terminal 2 — generate a code
cd core && cargo run -- generate --server ws://localhost:8080

# Terminal 3 — connect
cd core && cargo run -- connect <code> --server ws://localhost:8080

# File transfer test
cd core && cargo run -- send testfile.txt --server ws://localhost:8080
cd core && cargo run -- receive <code> --dest /tmp --server ws://localhost:8080
```

---

## Project Structure

```
punch/
├── core/src/
│   ├── main.rs        # CLI entry point and command definitions
│   ├── cli.rs         # Command handlers
│   ├── punch.rs       # Hole punching engine
│   ├── stun.rs        # STUN NAT discovery
│   ├── signaling.rs   # WebSocket signalling client
│   ├── crypto.rs      # X25519 + ChaCha20 encryption
│   ├── transfer.rs    # File transfer engine
│   ├── safety.rs      # Risk classification + consent
│   ├── token.rs       # Token generation
│   ├── token_store.rs # Token persistence + enforcement
│   └── logger.rs      # Local session logging
└── server/
    ├── cmd/main.go
    └── internal/
        ├── signaling/hub.go   # Session matchmaking
        ├── relay/             # Encrypted relay (v0.2+)
        └── config/config.go   # Server configuration
```

---

## What to Work On

Check the [ROADMAP](ROADMAP.md) for the current milestone.
Focus on what's in the next version — don't skip ahead.

**Actively needed:**
- v0.9 `punch-core` crate extraction and clean Rust API design
- v0.10 Sidecar architecture and REST/WS interface planning
- Windows build testing and bug reports
- ARM (Raspberry Pi) testing

**Good first contributions:**
- Improve error messages for common failure cases
- Add more file extension risk classifications in `safety.rs`
- Write integration tests for file transfer
- Test with different NAT configurations and report findings

---

## Code Style

**Go:**
- `gofmt` before committing
- Keep functions small and single-purpose
- Comment the *why* not the *what*
- No external dependencies without discussion

**Rust:**
- `rustfmt` before committing
- `cargo clippy -- -D warnings` must pass
- Use `?` for error propagation
- Prefer `async/await` over raw futures
- No `unwrap()` in production paths — use `?` or handle explicitly

**General:**
- One PR per concern
- Include a test if the change has testable behaviour
- Update relevant docs if behaviour changes
- Reference the ROADMAP milestone your PR addresses

---

## Philosophy Rules for PRs

These are non-negotiable:

| Rule | Reason |
|------|--------|
| No central data storage | Zero knowledge is the foundation |
| No account requirement | Ephemeral by design |
| No complexity without use case | Keep it simple |
| No breaking zero-knowledge server model | Core guarantee |
| No external runtime dependencies in CLI | Single binary distribution |

---

## Opening a PR

1. Fork the repo
2. Create a branch: `git checkout -b feat/your-feature`
3. Make your changes
4. Run `cargo clippy -- -D warnings` and `go vet ./...`
5. Commit: `git commit -m "feat: description"`
6. Push and open a PR against `main`

---

## Commit Convention

```
feat: add something new
fix: fix a bug
docs: documentation only
refactor: code change, no behaviour change
test: add or update tests
ci: CI/CD changes
```

---

## Reporting Issues

Include:
- OS and version
- Punch version (`punch --version`)
- Network type (WiFi / mobile / corporate)
- Full error output with `--verbose` flag
- Server logs if relevant

---

*Punch is zero-profit. Contributors own their contributions under MIT.*