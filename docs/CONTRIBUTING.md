# Contributing to Punch

Thanks for wanting to contribute. Punch is open source and zero-profit — contributions keep it alive.

## Before You Start

Read the [SRS](SRS.md) and [ROADMAP](ROADMAP.md) first. Understand what Punch is trying to be before adding to it.

The core philosophy: **simple, zero-knowledge, ephemeral.** If a contribution makes Punch more complex without a strong reason, it probably doesn't belong in v1.

## Setting Up

```bash
git clone https://github.com/MannanSaood/punch
cd punch

# Server (Go)
cd server && go mod download

# Core CLI (Rust)
cd core && cargo build
```

## Running Locally

```bash
# Terminal 1: Start the server
cd server && go run cmd/main.go

# Terminal 2: Generate a code
cd core && cargo run -- generate --server ws://localhost:8080

# Terminal 3: Connect
cd core && cargo run -- connect <code> --server ws://localhost:8080
```

## What to Work On

Check the ROADMAP for the current milestone. Focus on what's in the current version — don't skip ahead.

Good first issues:
- STUN server fallback logic
- Q-No usage counter enforcement
- Windows build testing
- Dashboard UI components

## Code Style

- **Go**: `gofmt`, keep functions small, comment the *why* not the *what*
- **Rust**: `rustfmt`, use `?` for error propagation, prefer `async/await` over raw futures
- **Svelte**: Keep components single-responsibility

## Philosophy Rules for PRs

1. Does it store user data anywhere central? **Reject.**
2. Does it require a user account? **Reject.**
3. Does it add complexity without a real use case? **Discuss first.**
4. Does it break the zero-knowledge server model? **Reject.**

## Opening a PR

- One PR per concern
- Include a test if possible
- Update the relevant docs if behaviour changes
- Reference the ROADMAP milestone your PR addresses
