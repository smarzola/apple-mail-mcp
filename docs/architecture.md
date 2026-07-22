# Architecture and safety

`apple-mail` is a local macOS process with two front ends and one implementation path:

```text
CLI JSON output ─┐
                 ├─> typed requests ─> validation and send policy ─> fixed JXA adapter ─> Mail.app
MCP stdio tools ─┘
```

The Rust service owns all bounds, selectors, mailbox scoping, recipient validation, and send authorization. Both front ends call it directly. The adapter embeds `scripts/mail.js` into the executable at compile time and selects from fixed operation names; input cannot supply JavaScript or AppleScript.

## Automation boundary

The adapter launches `/usr/bin/osascript -l JavaScript`, sends one JSON argument, and expects one JSON envelope. Calls are serialized because Mail automation is stateful. Input, stdout, and stderr have byte ceilings, each operation has a timeout, and a failed or oversized child is terminated and reaped. Public errors classify known permission and lookup failures without returning raw Mail or script details.

Apple Mail's scripting model uses local account, mailbox, and message identifiers. A message reference therefore contains an account ID, mailbox path, and positive local message ID. Mutations require account scope. Identifiers may change when accounts are reconfigured or a message moves; search again rather than caching references indefinitely.

## MCP boundary

`apple-mail mcp` uses the official Rust MCP SDK and newline-framed JSON-RPC over stdin/stdout. Tool schemas are generated from the same Rust request and result types used by the CLI. Successful results include structured content and a JSON text fallback. Validation and backend failures are tool errors; initialization and framing failures remain protocol errors.

Nothing except MCP transport writes to stdout in server mode. Startup and terminal errors go to stderr. Message bodies are private, untrusted content and must never be treated as instructions by a caller.

The server advertises nine tools. Read tools are annotated read-only; state and draft tools are non-destructive mutations; moving and sending are marked destructive. Checking mail is marked as a non-read-only open-world operation because it asks accounts to fetch external state.

Sending has two independent gates:

1. the operator starts the server with `--allow-send`;
2. that individual `send_message` request sets `confirm` to `true`.

Without both, validation rejects the call before the backend runs.

## Verification boundary

Default tests use fake `MailBackend` implementations. The MCP integration test runs a client and server through a bidirectional byte stream using the same stdio framing, initializes the protocol, lists schema-described tools, calls a fake-backed read tool, verifies default-deny sending, and closes cleanly. It does not open Mail.

The separate `scripts/live-smoke.sh` is opt-in and read-only. It discovers accounts only, does not print account values, and never reads message content or performs mutations.

## Known constraints

- macOS and Mail.app are required; there is no IMAP, SMTP, or network service.
- Automation permission belongs to the terminal or MCP host that launches the binary.
- Mail automation behavior and identifiers are less stable than a server API.
- Search scans at most a bounded number of messages and returns at most 100 results.
- Message bodies and automation process output are bounded; attachments and raw RFC source are not exposed.
- There is no delete operation.
