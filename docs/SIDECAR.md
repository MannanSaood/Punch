<p align="center">
  <a href="../README.md"><img src="assets/punch-logo.svg" width="520" alt="Punch"></a>
</p>

# Punch Sidecar API

`punch-sidecar` is the local HTTP/WebSocket adapter for `punch-core`. It lets a desktop app, script, or service start non-interactive Punch operations, inspect named sessions, cancel them, and receive progress events without parsing terminal output.

This document describes the API implemented in Punch v0.10.0. All examples assume the sidecar is listening on `127.0.0.1:7778` and that `PUNCH_TOKEN` contains the capability printed at startup.

## Architecture and security boundary

```text
local client -> HTTP/WebSocket on 127.0.0.1 -> punch-sidecar -> punch-core
                                                        |-> signalling service
                                                        `-> direct or encrypted-relay peer transport
```

The sidecar is a process-local control plane. It owns in-memory session state and cancellation handles; `punch-core` owns token persistence, signalling, peer connectivity, file I/O, and transport cleanup.

Punch's zero-knowledge properties remain unchanged:

- after a local client supplies file or pipe data to the sidecar, peer payloads travel through Punch transports rather than the signalling server;
- relay payloads are encrypted end to end, so a relay can forward bytes but cannot decrypt them;
- token enforcement and durable Q-No/P-No token state remain on the Punch device;
- the signalling service is an ephemeral matchmaker and does not provide accounts or durable user/session storage.

“Zero knowledge” does not mean that every component sees nothing. The local sidecar necessarily sees API requests, local paths, pipe bytes, and the four-digit code supplied by its client. The signalling service sees transient matchmaking data, including a code and peer connection metadata. A compromised local user or process that obtains the bearer token can control the sidecar.

### Local binding and authorization

The listener is hard-coded to IPv4 loopback (`127.0.0.1`) and cannot be configured to bind publicly. Every route, including `/health`, requires the random per-process capability printed on stdout:

```http
Authorization: Bearer <capability>
```

The capability changes on each start. Do not log it, put it in a URL, pass it to an untrusted subprocess, or expose the sidecar through a reverse proxy. Other processes running as the same OS user may still be able to read captured output or make local requests, so normal host isolation remains important.

Browser requests must also have an `http` or `https` Origin whose host is `localhost`, `127.0.0.1`, or `::1`. Other origins receive `403 forbidden_origin`. CORS permits only loopback origins, the `GET`, `POST`, `DELETE`, and `OPTIONS` methods, and the `Content-Type` and `Authorization` headers.

## Installation and startup

GitHub releases contain both the CLI and standalone sidecar:

| Platform | CLI | Standalone sidecar |
|---|---|---|
| Windows x86-64 | `punch-windows-x86_64.exe` | `punch-sidecar-windows-x86_64.exe` |
| Linux x86-64 | `punch-linux-x86_64` | `punch-sidecar-linux-x86_64` |
| macOS Intel | `punch-macos-x86_64` | `punch-sidecar-macos-x86_64` |
| macOS ARM | `punch-macos-arm64` | `punch-sidecar-macos-arm64` |

On Linux/macOS, make the downloaded binary executable. From source:

```bash
cd core
cargo build --locked --release -p punch-cli -p punch-sidecar
```

The binaries are written to `core/target/release/punch` and `core/target/release/punch-sidecar` (with `.exe` on Windows).

Start the standalone service:

```bash
punch-sidecar --port 7778 --server wss://129.159.21.6.nip.io
```

Or start the same service through the CLI:

```bash
punch sidecar --port 7778 --server wss://129.159.21.6.nip.io
# The global option is also accepted before the subcommand:
punch --server wss://signal.example.com sidecar --port 9000
```

`--port` defaults to `7778`; `0` asks the OS for an ephemeral port. `--server` defaults to `wss://129.159.21.6.nip.io`. Startup prints the actual HTTP address and bearer token. If a requested port is occupied, startup fails with a message suggesting another `--port` value. Ctrl+C triggers graceful shutdown.

Shell setup example:

```bash
export PUNCH_URL=http://127.0.0.1:7778
export PUNCH_TOKEN='<capability printed by punch-sidecar>'
```

PowerShell:

```powershell
$env:PUNCH_URL = 'http://127.0.0.1:7778'
$env:PUNCH_TOKEN = '<capability printed by punch-sidecar>'
```

## Data model

### Session snapshot

Session detail and list routes return this full structure:

```json
{
  "name": "report-send",
  "code": "4829",
  "token_type": "T-No",
  "state": "active",
  "operation": "send_file",
  "created_at": "2026-09-14T10:00:00Z",
  "updated_at": "2026-09-14T10:00:02Z",
  "last_error": null,
  "progress": {
    "completed": 524288,
    "total": 1048576,
    "unit": "bytes"
  },
  "metadata": {}
}
```

`code`, `last_error`, and `progress` may be `null`. When present, `progress.total` and `progress.unit` may also be `null`. Timestamps are UTC RFC 3339 strings. Valid `token_type` values are `T-No`, `Q-No`, and `P-No`; valid `operation` values are `connect`, `generate`, `send_file`, `receive_file`, `forward`, `forward_connect`, `pipe_send`, `pipe_receive`, and `shell`.

Operation-start, consent, and cancel routes return a smaller operation response:

```json
{"name":"report-send","code":"4829","token_type":"T-No","state":"waiting"}
```

The `code` member is omitted when it is not known; it is not serialized as `null` in this response type.

### Names and request validation

A session name is 1–64 ASCII letters, digits, `.`, `_`, or `-`, and must begin and end with a letter or digit. Names are unique until deleted. Codes are exactly four ASCII digits. JSON operation request types reject unknown fields. JSON bodies and pipe-send request bodies are limited to 65,536 bytes.

## REST endpoints

Every example below also requires `Authorization: Bearer $PUNCH_TOKEN`.

| Method and path | Request | Success |
|---|---|---|
| `GET /health` | none | `200` health object |
| `GET /sessions` | none | `200` array of session snapshots, sorted by name |
| `POST /sessions` | create-session JSON | `201` session snapshot |
| `GET /session/{name}` | none | `200` session snapshot |
| `DELETE /session/{name}` | none | `200` removed snapshot |
| `POST /session/{name}/generate` | token-options JSON | `200` operation response |
| `POST /session/{name}/connect` | connect JSON | `202` operation response |
| `POST /session/{name}/send` | send JSON | `202` operation response |
| `POST /session/{name}/receive` | receive JSON | `202` operation response in `waiting_for_consent` |
| `POST /session/{name}/forward` | forward JSON | `202` operation response in `waiting_for_consent` |
| `POST /session/{name}/forward-connect` | forward-connect JSON | `202` operation response in `waiting_for_consent` |
| `POST /session/{name}/pipe-send` | binary body plus query options | `202` operation response |
| `POST /session/{name}/pipe-recv` | pipe-receive JSON | `200` byte stream, or `403` rejected operation response |
| `POST /session/{name}/consent` | consent JSON | `202` accepted start or `200` rejected response |
| `POST /session/{name}/cancel` | no body required | `200` cancelled operation response |
| `GET /ws` | WebSocket upgrade | `101` WebSocket connection |

### Health and sessions

`GET /health` returns:

```json
{"status":"ok","service":"punch-sidecar"}
```

`POST /sessions` creates bookkeeping state without starting a core operation:

```json
{
  "name": "external-job",
  "code": null,
  "token_type": "T-No",
  "operation": "connect",
  "metadata": {"owner": "desktop-app"}
}
```

`code` and `metadata` are optional. This low-level endpoint is useful for clients that need session records, but there is no route to attach a core operation to a manually created record.

`DELETE /session/{name}` removes the record. A nonterminal session is changed to `cancelled`, its operation is cancelled, and deletion waits up to three seconds for task cleanup before returning. A terminal session retains its terminal state in the returned snapshot. The name can be reused after cleanup.

### Generate and token options

```json
{"uses": null, "permanent": false}
```

Both fields are optional. The combinations are:

| Request | Result |
|---|---|
| `{}` or `{"uses":null,"permanent":false}` | T-No |
| `{"uses":3}` | Q-No with three uses |
| `{"permanent":true}` | P-No |

`uses` must be greater than zero. Combining a non-null `uses` with `permanent: true` returns `400 invalid_request`.

`POST /session/{name}/generate` creates the token and returns its four-digit code synchronously with state `waiting`. It does not start a background listen operation. The same token options are flattened into send and forward JSON, and are query parameters for pipe-send.

### Connect

```json
{"code":"4829","log":false}
```

`log` is optional and defaults to `false`. The operation runs asynchronously after the `202` response.

### File send and receive

Send request:

```json
{"path":"C:/data/report.pdf","uses":null,"permanent":false}
```

`path` is interpreted on the sidecar host and must already be a regular file. Receive request:

```json
{"code":"4829","destination":"C:/data/downloads"}
```

`destination` is interpreted on the sidecar host. It must be a nonempty directory path and must not name an existing regular file; the core is responsible for the transfer-specific directory behavior.

Receive is intentionally two-step. Its first response is `waiting_for_consent`; the caller must inspect the request in its own UI/policy and then call the consent route.

### Port forwarding

Expose request:

```json
{"port":8096,"protocol":"tcp","uses":null,"permanent":false}
```

`protocol` is `tcp`, `udp`, or `both`; `port` must be 1–65535. Connect request:

```json
{"code":"4829","local_port":8097}
```

`local_port` is optional. If omitted, the core chooses a loopback port. If supplied, it must be 1–65535. Both routes use the explicit consent workflow before starting.

### Consent

Receive, forward, and forward-connect store their validated request and enter `waiting_for_consent`. Resolve them with exactly one of:

```json
{"decision":"accept"}
```

```json
{"decision":"reject"}
```

Acceptance moves through `accepted` to `waiting`, schedules the core operation, and returns `202`. Rejection moves directly to terminal state `rejected` and returns `200`. A second decision, or consent on any session without a pending request, returns `409 consent_not_pending`.

Pipe receive is different: its initial JSON body includes `decision`. An accepted request immediately returns the byte stream; a rejected request returns `403` and a JSON operation response with state `rejected`.

### Cancellation and deletion

`POST /session/{name}/cancel` cancels the operation but keeps its snapshot available with state `cancelled`. `DELETE /session/{name}` cancels if necessary and removes the snapshot. Cancellation is cooperative for up to two seconds, after which an unresponsive core task is aborted. Graceful process shutdown cancels all live sessions, allows up to five seconds for cleanup, closes WebSockets, and then aborts stragglers.

### Pipe streaming and backpressure

Pipe send:

```http
POST /session/stdin/pipe-send?uses=3&permanent=false
Content-Type: application/octet-stream

<bytes>
```

The query fields follow the token-options rules. The request returns `202`; the body is consumed by the background operation. The entire HTTP request body is capped at 64 KiB in the current implementation.

Pipe receive:

```json
{"code":"4829","decision":"accept"}
```

An accepted request returns `200 Content-Type: application/octet-stream` with a streaming response body. The bridge uses a bounded 64 KiB duplex buffer and does not aggregate the response. Slow HTTP consumers naturally backpressure the core writer. A disconnected receiver closes the writer and terminates the operation; cancellation propagates to the core. Pipe-send likewise adapts the HTTP body as a stream rather than collecting it first, subject to the 64 KiB total request limit.

## HTTP errors

Errors use a stable envelope:

```json
{"error":{"code":"invalid_request","message":"code must contain exactly four digits"}}
```

| Status | Codes and meaning |
|---|---|
| `400 Bad Request` | `invalid_request`, `invalid_session_name` |
| `401 Unauthorized` | `unauthorized`: missing or invalid capability |
| `403 Forbidden` | `forbidden_origin`; pipe-receive rejection instead returns an operation response |
| `404 Not Found` | `session_not_found`, `route_not_found` |
| `409 Conflict` | `duplicate_session`, `invalid_state_transition`, `operation_conflict`, `consent_not_pending` |
| `413 Payload Too Large` | `payload_too_large` |
| `429 Too Many Requests` | `resource_limit`: active-session or WebSocket-client cap reached |
| `500 Internal Server Error` | `core_operation_failed` |
| `501 Not Implemented` | `not_implemented` (defined for unavailable operations) |
| `503 Service Unavailable` | `shutting_down` |

The sidecar permits at most 256 active sessions, retains at most 1,024 terminal snapshots, and permits at most 32 WebSocket clients.

## WebSocket events

Connect to `ws://127.0.0.1:7778/ws` using the subprotocol `punch-token.<capability>`. The credential is deliberately not accepted in the URL. On success the server selects the same subprotocol.

Every text frame is JSON:

```json
{
  "sequence": 12,
  "event": "transfer_progress",
  "name": "report-send",
  "timestamp": "2026-09-14T10:00:02Z",
  "data": {"percent":50,"bytes_completed":524288,"bytes_total":1048576}
}
```

`sequence` is monotonic and process-local, restarting from the new process. `name` is omitted for connection-wide events. Currently emitted events are:

| Event | `data` |
|---|---|
| `connected` | `{"sessions":[{"name","state","operation","token_type"}, ...]}` sent immediately to a new subscriber |
| `session_created` | session state summary |
| `session_state_changed` | session state summary, or `{"state":"<core state text>"}` for a forwarded core state event |
| `session_connected` | session state summary |
| `session_closed` | session state summary |
| `session_failed` | session state summary plus sanitized `error` |
| `session_cancelled` | session state summary |
| `transfer_progress` | `percent`, `bytes_completed`, `bytes_total` |
| `forward_stream` | `count` |
| `resync_required` | `dropped_events` and current non-sensitive session summaries |

A session state summary contains `state`, `operation`, and `token_type`. Events omit codes, paths, metadata, pipe content, peer messages, and connection payloads.

Each subscriber has a 256-event broadcast buffer. Delivery is intentionally lossy rather than allowing an unbounded queue. If a client falls behind, it receives `resync_required` and should refresh with `GET /sessions` or `GET /session/{name}`. A socket that cannot accept an event for five seconds is closed. Ping frames receive pong frames; other client frames are ignored.

## Session state machine

Terminal states are `closed`, `rejected`, `failed`, and `cancelled`.

```text
created -> waiting -> connected -> active -> closed
   |          |           |          |
   |          +-----------+----------+-> failed/cancelled
   |
   `-> waiting_for_consent -> accepted -> waiting -> ...
                  |             `-> active -> closed   (pipe receive)
                  `-> rejected
```

Some operations legitimately skip states: a successful asynchronous operation may go from `waiting` to `active` to `closed`; pipe receive goes from `accepted` directly to `active`; failures can terminate several nonterminal states. Clients should treat the state as observational and must not assume that every intermediate event will be delivered.

## T-No, Q-No, and P-No

- T-No is selected by default and is intended for one operation/session.
- Q-No is selected with a positive `uses` count. The durable remaining-use state is managed locally by `punch-core`.
- P-No is selected with `permanent: true`. P-No verification remains a separate CLI workflow (`punch verify <code>`); the sidecar has no verify or revoke route.

The sidecar snapshot records only the token kind and current four-digit code. It does not expose remaining Q-No uses, P-No verification state, or the token store. Deleting a sidecar session does not revoke a durable Q-No/P-No token.

## curl examples

```bash
# Health and session list
curl -H "Authorization: Bearer $PUNCH_TOKEN" "$PUNCH_URL/health"
curl -H "Authorization: Bearer $PUNCH_TOKEN" "$PUNCH_URL/sessions"

# Connect asynchronously, inspect, then cancel
curl -X POST "$PUNCH_URL/session/laptop/connect" \
  -H "Authorization: Bearer $PUNCH_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"code":"4829","log":false}'
curl -H "Authorization: Bearer $PUNCH_TOKEN" "$PUNCH_URL/session/laptop"
curl -X POST -H "Authorization: Bearer $PUNCH_TOKEN" \
  "$PUNCH_URL/session/laptop/cancel"

# Start receive, then explicitly accept
curl -X POST "$PUNCH_URL/session/report/receive" \
  -H "Authorization: Bearer $PUNCH_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"code":"4829","destination":"./downloads"}'
curl -X POST "$PUNCH_URL/session/report/consent" \
  -H "Authorization: Bearer $PUNCH_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"decision":"accept"}'

# Stream up to 64 KiB into a pipe
curl --data-binary @small-input.bin -X POST \
  "$PUNCH_URL/session/pipe-out/pipe-send?uses=3" \
  -H "Authorization: Bearer $PUNCH_TOKEN" \
  -H 'Content-Type: application/octet-stream'

# Receive a pipe response without buffering it in curl
curl --no-buffer -X POST "$PUNCH_URL/session/pipe-in/pipe-recv" \
  -H "Authorization: Bearer $PUNCH_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"code":"4829","decision":"accept"}' \
  --output received.bin
```

## Python example

This uses only the standard library:

```python
import json
import os
import urllib.request

base = os.environ.get("PUNCH_URL", "http://127.0.0.1:7778")
token = os.environ["PUNCH_TOKEN"]

def request(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(
        base + path,
        data=data,
        method=method,
        headers={
            "Authorization": f"Bearer {token}",
            **({"Content-Type": "application/json"} if data else {}),
        },
    )
    with urllib.request.urlopen(req) as response:
        return response.status, json.load(response)

status, operation = request(
    "POST", "/session/python-connect/connect", {"code": "4829", "log": False}
)
print(status, operation)  # 202, then poll the snapshot
_, snapshot = request("GET", "/session/python-connect")
print(snapshot["state"])
```

For WebSockets, install `websockets` and pass the capability as a subprotocol:

```python
import asyncio, json, os
import websockets

async def events():
    token = os.environ["PUNCH_TOKEN"]
    async with websockets.connect(
        "ws://127.0.0.1:7778/ws",
        subprotocols=[f"punch-token.{token}"],
    ) as socket:
        async for message in socket:
            print(json.loads(message))

asyncio.run(events())
```

## Node.js example

Node.js 18 or newer provides `fetch`:

```js
const base = process.env.PUNCH_URL ?? "http://127.0.0.1:7778";
const token = process.env.PUNCH_TOKEN;

async function request(method, path, body) {
  const response = await fetch(base + path, {
    method,
    headers: {
      authorization: `Bearer ${token}`,
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = await response.json();
  if (!response.ok) throw new Error(`${response.status}: ${JSON.stringify(payload)}`);
  return payload;
}

const operation = await request("POST", "/session/node-send/send", {
  path: "C:/data/report.pdf",
  uses: 2,
  permanent: false,
});
console.log(operation);
console.log(await request("GET", "/session/node-send"));
```

For events, install `ws` with `npm install ws`:

```js
import WebSocket from "ws";

const token = process.env.PUNCH_TOKEN;
const socket = new WebSocket(
  "ws://127.0.0.1:7778/ws",
  [`punch-token.${token}`],
);
socket.on("message", data => console.log(JSON.parse(data.toString())));
```

## Go example

This standard-library example starts and polls an operation:

```go
package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "io"
    "net/http"
    "os"
)

func call(method, path string, body any) ([]byte, error) {
    base := os.Getenv("PUNCH_URL")
    if base == "" { base = "http://127.0.0.1:7778" }
    var input io.Reader
    if body != nil {
        encoded, err := json.Marshal(body)
        if err != nil { return nil, err }
        input = bytes.NewReader(encoded)
    }
    req, err := http.NewRequest(method, base+path, input)
    if err != nil { return nil, err }
    req.Header.Set("Authorization", "Bearer "+os.Getenv("PUNCH_TOKEN"))
    if body != nil { req.Header.Set("Content-Type", "application/json") }
    response, err := http.DefaultClient.Do(req)
    if err != nil { return nil, err }
    defer response.Body.Close()
    payload, err := io.ReadAll(response.Body)
    if err != nil { return nil, err }
    if response.StatusCode < 200 || response.StatusCode >= 300 {
        return nil, fmt.Errorf("sidecar %s: %s", response.Status, payload)
    }
    return payload, nil
}

func main() {
    started, err := call("POST", "/session/go-forward/forward", map[string]any{
        "port": 8096, "protocol": "tcp", "permanent": false,
    })
    if err != nil { panic(err) }
    fmt.Println(string(started))

    accepted, err := call("POST", "/session/go-forward/consent", map[string]string{
        "decision": "accept",
    })
    if err != nil { panic(err) }
    fmt.Println(string(accepted))
}
```

For a pipe response, keep `response.Body` open and copy it directly to the destination with `io.Copy`; do not call `io.ReadAll`. A Go WebSocket client must offer `punch-token.<capability>` as a subprotocol; the standard library has no WebSocket client, so use a WebSocket package of your choice.

## Troubleshooting

- `401 unauthorized`: copy the token from the current sidecar process and use the exact case-sensitive `Bearer ` prefix. Restarting invalidates the old token.
- `403 forbidden_origin`: serve browser code from `http(s)://localhost`, `127.0.0.1`, or `::1`. Native clients should normally omit `Origin`.
- WebSocket handshake fails: supply `punch-token.<token>` in `Sec-WebSocket-Protocol`, not `Authorization` in browser code and not a query string.
- Address already in use: choose another `--port`; update the client's base URL.
- `400 invalid_request`: check four-digit codes, nonzero ports, token-option combinations, paths, JSON types, and unknown JSON fields.
- `409 duplicate_session`: use a unique name or delete the old session after its operation has cleaned up.
- `409 consent_not_pending`: consent is only valid once for receive, forward, or forward-connect sessions currently awaiting it.
- `413 payload_too_large`: JSON and pipe-send requests currently have a 64 KiB limit.
- Events appear to skip: handle `resync_required` and refresh state over REST. WebSocket delivery is not a durable event log.
- Operation remains `waiting`: this can be normal while waiting for a peer. A `generate` operation only generates a token; use the appropriate sender/listener workflow to accept a peer.
- Network connection fails: verify both peers use the same signalling server and that its URL is reachable; mobile/corporate networks may require the encrypted relay path.

## Known limitations

- The API has no version prefix and no OpenAPI document; compatibility is tied to the Punch release.
- Authentication is a transient stdout capability. There is no token file, rotation endpoint, TLS listener, or remote bind mode.
- There are no sidecar routes for `listen`, token listing, P-No verification, revocation, dashboard control, or remote shell.
- `POST /sessions` creates state only and cannot attach a later core operation.
- Generate creates a token but does not run a reusable-token listener.
- Pipe-send is streamed internally but capped at 64 KiB total input, so it is not yet suitable for large or indefinite streams.
- WebSocket events are live, bounded, and lossy; there is no replay across lag or process restart.
- Core consent detail events, peer messages, retries, warnings, and completed-event payloads are deliberately not forwarded. Consent is driven by REST state and explicit caller policy.
- Session state is in memory and disappears when the sidecar exits. Q-No/P-No state managed by `punch-core` is separate and may persist locally.
- Release binaries currently cover Windows x86-64, Linux x86-64, macOS Intel, and macOS ARM only.
