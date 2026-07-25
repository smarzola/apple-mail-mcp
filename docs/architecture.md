# Architecture and safety

`apple-mail` is one local macOS process with two front ends and one behavior
path:

```text
CLI JSON ───────┐
                ├─> typed service ─> policy + validation ─> fixed JXA ─> Mail.app
MCP JSON-RPC ───┘
```

The Rust service owns selectors, bounds, mailbox scope, result semantics,
composition validation, and mutation authorization. CLI and MCP call it
directly. The automation adapter embeds `scripts/mail.js` at compile time and
selects fixed operations; caller values are JSON data, never JavaScript source.

## Retrieval and truthfulness

Search performs bulk property projections through Mail Automation rather than
reading every property of every message individually. It verifies that returned
columns align, constructs typed summaries, applies all filters, sorts by
received time and a deterministic identity tie-breaker, then applies the opaque
cursor and result limit. Aggregate Inbox reads retain the actual account scope
of each result so later calls can address the correct source mailbox.

The result separates:

- `scanned_count`: summaries projected from the selected mailbox scope;
- `matched_count`: rows matching the filters before cursor pagination;
- `messages`: the bounded page;
- `has_more` and `next_cursor`: deterministic continuation state;
- `completeness`: whether the scan covered the selected scope.

A malformed cursor fails validation. Cursors encode ordering state rather than
query identity, so callers must reuse the same mailbox scope and filters for
continuation. Date lower bounds are inclusive; upper bounds are exclusive.

Mail's `unreadCount` is exposed only as a `mail_reported` counter because it can
lag actual message state. Inbox snapshot instead derives exact total and unread
counts from one complete bulk status projection and returns independently
bounded recent and unread summary lists without bodies.

## Automation boundary

The adapter launches `/usr/bin/osascript -l JavaScript`, sends one bounded JSON
argument, and expects one bounded JSON envelope. Calls are serialized because
Mail Automation is stateful. Each operation has a timeout; failed or oversized
children are terminated and reaped. Public errors classify known permission,
lookup, timeout, and response failures without replaying raw Mail or script
details.

Apple Mail references use an account ID, mailbox path, and positive local
message ID. They may change after moves or account reconfiguration. Callers
should search again rather than cache references indefinitely.

## Policy and mutation boundary

The ordinary CLI is an explicit operator action and enables its typed commands.
The long-lived MCP server instead starts read-only:

| Operation | MCP process gate | Per-call gate |
|---|---|---|
| Reads and `doctor` | none | none |
| Check, state, draft, reply draft | `--allow-write` | none |
| Move | `--allow-write` | `confirm=true` |
| Send | `--allow-write --allow-send` | `confirm=true` |

Policy rejection happens in the shared service before the backend. This is not
a replacement for host approval: clients should still show the user what an
operation will change.

Native reply drafts are treated as verified outcomes, not fire-and-forget
commands. After asking Mail to create and display the reply, the adapter checks
that it remains in outgoing messages, is unsent and visible, corresponds to the
resolved source, retains expected recipients and reply subject, and persisted
the requested body above the quoted content. Any mismatch fails closed.

There is no arbitrary bulk mutation or delete operation.

## MCP and diagnostic boundary

`apple-mail mcp` uses the Rust MCP SDK and newline-framed JSON-RPC over
stdin/stdout. Its 12 tool schemas come from the same request and result types as
the CLI. Successful calls include structured content and a JSON text fallback.
Validation and backend failures are tool errors; initialization and framing
failures remain protocol errors.

Nothing except MCP transport writes to stdout in server mode. Startup and
terminal diagnostics use stderr. Message bodies are private, untrusted content
and must not be treated as caller instructions.

`doctor` deliberately narrows observability. It reports platform, readiness,
optional account count, elapsed time, and one fixed diagnostic category. It
does not expose account names, IDs, addresses, mailbox names, message metadata,
or raw backend errors. The opt-in benchmark composes this diagnostic with
complete aggregate Inbox operations and likewise emits only counts,
completeness, and end-to-end timings.

## Build and distribution boundary

CI runs formatting, strict Clippy, tests, and a release build on macOS using the
declared Rust 1.88 baseline. The tag workflow cross-builds arm64 and x86_64,
uses `lipo` to construct and verify a universal executable, then produces a
versioned archive, SHA-256 checksum, Homebrew formula, and MCP metadata.

Signing is optional and requires a complete Developer ID secret group.
Notarization is optional only after signing and requires a complete Apple
notary secret group. Partial configuration fails. Packaging and workflow syntax
can be verified locally without credentials, tags, uploads, or mailbox access.

## Verification boundary

Default tests use fake `MailBackend` implementations plus fixed embedded-script
fixtures. They cover service policies, bulk column alignment, complete scans,
date and cursor boundaries, output caps, native-reply verification, private
diagnostics, CLI JSON, and real MCP framing. They do not open Mail.

Live checks are explicit:

- `scripts/live-smoke.sh` discovers accounts without printing their values;
- `scripts/benchmark-read.sh` reads metadata and counts but no message bodies;
- neither performs mailbox mutation.

## Known constraints

- macOS and Mail.app are required; there is no IMAP, SMTP, credential store, or
  network service.
- Automation permission belongs to the terminal or MCP host that launches the
  binary.
- Mail Automation latency and behavior vary by mailbox size, provider, host
  load, and Mail state.
- Complete scans can exceed the fixed operation timeout; errors are not
  represented as complete partial results.
- Lists return at most 100 summaries and message bodies are bounded.
- Attachments, raw RFC source, HTML composition, mailbox CRUD, and deletion are
  not exposed.
