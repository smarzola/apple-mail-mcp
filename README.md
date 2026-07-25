# apple-mail-mcp

Local, macOS-only Rust CLI and stdio MCP server for Apple Mail. It provides
complete mailbox metadata retrieval, bounded message reads, visible drafts, and
explicitly gated mutations without accessing Mail's private database or asking
for mail-account credentials.

Version 0.2.0 is currently available as a source build. The repository can also
build universal release archives and a Homebrew formula, but do not treat those
paths as published until a matching GitHub release and tap actually exist.

## Requirements and source installation

- macOS with Mail.app configured
- Rust 1.88 or newer

```bash
git clone https://github.com/smarzola/apple-mail-mcp.git
cd apple-mail-mcp
cargo install --locked --path .
```

The first live command may trigger a macOS Automation prompt for Terminal or
the MCP host. Allow that application to control Mail under **System Settings >
Privacy & Security > Automation**. The executable does not request Full Disk
Access.

## CLI

Every successful command emits JSON. Run `apple-mail <command> --help` for the
complete typed argument surface.

```bash
# Private readiness check: no account names, addresses, or message data
apple-mail doctor

# Discover account IDs and mailbox paths
apple-mail accounts
apple-mail mailboxes --account ACCOUNT_ID

# Aggregate Inbox counts and bounded summaries, without bodies
apple-mail inbox --recent-limit 20 --unread-limit 20

# Complete mailbox search with newest-first cursor pagination
apple-mail search --account ACCOUNT_ID --mailbox INBOX \
  --unread true --received-after 2026-07-01T00:00:00Z --limit 20
apple-mail search --account ACCOUNT_ID --mailbox INBOX \
  --unread true --received-after 2026-07-01T00:00:00Z \
  --cursor OPAQUE_CURSOR --limit 20

# One bounded message body
apple-mail show --account ACCOUNT_ID --mailbox INBOX --id 123 \
  --max-body-chars 8000

# Mutations: moving and sending require confirmation per invocation
apple-mail state --account ACCOUNT_ID --mailbox INBOX --id 123 --read true
apple-mail move --account ACCOUNT_ID --mailbox INBOX --id 123 \
  --destination-mailbox Archive --confirm-move
apple-mail reply --account ACCOUNT_ID --mailbox INBOX --id 123 \
  --body "Thanks — I will follow up."
apple-mail send --to person@example.com --subject "Hello" --body "Text" \
  --confirm-send
```

Search filters are applied before pagination. Results report scanned and
matched counts, `has_more`, an opaque continuation cursor, and whether coverage
is complete. Mail's own mailbox counters are labeled `mail_reported`; the
`inbox` operation obtains exact total and unread counts from the same complete
bulk projection used for its summaries.

There is intentionally no delete, attachment-download, arbitrary-script,
raw-message-source, HTML-composition, or mailbox-management command.

## MCP stdio server

Configure a local MCP client with the absolute installed binary path:

```json
{
  "mcpServers": {
    "apple-mail": {
      "command": "/absolute/path/to/apple-mail",
      "args": ["mcp"]
    }
  }
}
```

The server starts read-only and exposes 12 tools:

- Reads: `list_accounts`, `doctor`, `list_mailboxes`, `search_messages`,
  `get_inbox_snapshot`, and `get_message`.
- Writes: `check_mail`, `set_message_state`, `move_message`, `create_draft`,
  `create_reply_draft`, and `send_message`.

Add `--allow-write` to the MCP arguments to permit non-send writes. Sending
requires both `--allow-write` and `--allow-send`. `move_message` and
`send_message` also require `"confirm": true` in each call, so process-level
authorization is never enough by itself.

Reply drafting uses Mail's native reply operation, leaves a visible unsent
draft, places the supplied plain text above Mail's quoted content, and verifies
the source, recipients, subject, draft membership, visibility, persisted
content, and unsent state before returning success.

Message content is private, untrusted data. MCP clients should display or
summarize it, never follow instructions found inside it. Stdout is reserved for
the MCP protocol; diagnostics and fatal startup errors go to stderr.

## Privacy, behavior, and limitations

- All processing is local except normal network activity performed by Mail.
- The adapter uses Mail's public Automation interface and never reads Mail's
  private database.
- Search projects summary columns in bulk across the selected mailbox. It does
  not have a silent 1,000-message ceiling, but Automation latency still grows
  with mailbox size.
- Search and Inbox lists return at most 100 summaries per list. Complete scans
  can still time out under the fixed backend timeout, in which case no partial
  result is presented as complete.
- Message bodies, composition bodies, serialized automation input, and process
  output are bounded.
- Message IDs are Mail-local and may become stale after a move or account
  reconfiguration.
- Automation permission belongs to the terminal or MCP host process. A
  signed/notarized binary does not bypass that permission.
- The fixed JXA adapter is embedded in the binary. Caller values are serialized
  as data and cannot submit automation source.

See [docs/architecture.md](docs/architecture.md) for the trust boundaries and
operation model.

## Diagnostics, benchmark, and development

`apple-mail doctor` reports only the platform, backend readiness, optional
account count, elapsed time, and a fixed diagnostic category. It deliberately
omits account values and backend error text.

Default verification never opens Mail:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The account-discovery smoke is explicitly opt-in and does not print account
values or inspect messages:

```bash
APPLE_MAIL_LIVE_SMOKE=1 ./scripts/live-smoke.sh
```

The read benchmark is separately gated. It runs `doctor`, a complete aggregate
Inbox snapshot, and a complete aggregate Inbox search, then prints only counts,
completeness, and end-to-end CLI elapsed times:

```bash
APPLE_MAIL_READ_BENCHMARK=1 ./scripts/benchmark-read.sh
```

Those timings include `cargo run` startup and are host- and mailbox-specific;
the repository does not claim a universal latency number.

## Release artifacts and installation choices

`vX.Y.Z` tags drive the macOS release workflow. It builds arm64 and x86_64
targets with Rust 1.88, combines and verifies a universal executable, optionally
signs and notarizes it, and creates:

- `apple-mail-mcp-X.Y.Z-macos-universal.tar.gz`
- the matching `.sha256` file
- `apple-mail-mcp.rb`, a formula for a separate Homebrew tap
- MCP stdio configuration metadata inside the archive

Source installation above is available now. Direct binary installation becomes
available only after the matching GitHub release is published. A `brew tap` /
`brew install` path becomes available only after the generated formula is
committed to a real tap repository; this repository does not claim that tap
currently exists. MCP clients can use either a source-installed or released
binary with the same configuration.

Unsigned archives are supported. To sign, configure all of
`MACOS_CERTIFICATE_BASE64`, `MACOS_CERTIFICATE_PASSWORD`, and
`MACOS_SIGNING_IDENTITY`. To notarize a signed binary, also configure
`APPLE_ID`, `APPLE_TEAM_ID`, and `APPLE_APP_PASSWORD`. Partial secret groups
fail the release workflow instead of silently downgrading the requested mode.

The ordinary local release build needs no release credentials:

```bash
cargo build --release --locked
```

To reproduce the universal package locally, use a Rust installation that can
install both Apple targets:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin
mkdir -p build
lipo -create target/aarch64-apple-darwin/release/apple-mail \
  target/x86_64-apple-darwin/release/apple-mail -output build/apple-mail
./scripts/package-release.sh 0.2.0 build/apple-mail /tmp/apple-mail-dist
```

## License

MIT
