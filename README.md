# apple-mail-mcp

`apple-mail-mcp` gives command-line tools and Model Context Protocol (MCP)
clients controlled access to Apple Mail on macOS.

Use it to:

- Search message metadata across complete mailboxes and read bounded content.
- Get exact Inbox counts and recent or unread summaries.
- Create visible drafts and native reply drafts.
- Change message state or move messages with explicit authorization.
- Send mail only when both the operator and the individual request confirm it.

The installed command is named `apple-mail`. It uses Mail's public Automation
interface and doesn't read Mail's private database or require account
credentials.

## Before you begin

You need:

- macOS with Mail configured for at least one account.
- Rust 1.88 or later to install from source.
- Permission for your terminal or MCP client to control Mail.

The project currently supports installation from source. Published tags can
also provide a universal macOS archive and checksum on
[GitHub Releases](https://github.com/smarzola/apple-mail-mcp/releases).

## Install from source

Run the following commands:

```bash
git clone https://github.com/smarzola/apple-mail-mcp.git
cd apple-mail-mcp
cargo install --locked --path .
```

Verify the installation:

```bash
apple-mail --version
```

## Grant access to Mail

The first command that accesses Mail might open a macOS Automation prompt.
Allow the terminal or MCP client that launched `apple-mail` to control Mail.

To review or change this permission:

1. Open **System Settings**.
2. Click **Privacy & Security**.
3. Click **Automation**.
4. Enable Mail access for the application that launches `apple-mail`.

`apple-mail` doesn't require Full Disk Access.

Check the platform and Automation connection without exposing account values:

```bash
apple-mail doctor
```

## Configure an MCP client

Add a standard input/output (stdio) server to your MCP client configuration.
Replace `/absolute/path/to/apple-mail` with the output of
`command -v apple-mail`.

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

The MCP server starts in read-only mode.

### Allow mailbox changes

Add `--allow-write` to permit checking for new mail, changing message state,
moving messages, and creating drafts or reply drafts:

```json
{
  "mcpServers": {
    "apple-mail": {
      "command": "/absolute/path/to/apple-mail",
      "args": ["mcp", "--allow-write"]
    }
  }
}
```

Moving a message also requires `confirm: true` in that tool call.

### Allow sending

Add both `--allow-write` and `--allow-send`:

```json
{
  "mcpServers": {
    "apple-mail": {
      "command": "/absolute/path/to/apple-mail",
      "args": ["mcp", "--allow-write", "--allow-send"]
    }
  }
}
```

Every `send_message` call must also contain `confirm: true`. Starting the
server with send permission isn't sufficient by itself.

### Available MCP tools

The server provides the following read tools:

- `list_accounts`
- `doctor`
- `list_mailboxes`
- `search_messages`
- `get_inbox_snapshot`
- `get_message`

The server provides the following write tools:

- `check_mail`
- `set_message_state`
- `move_message`
- `create_draft`
- `create_reply_draft`
- `send_message`

All tool inputs and outputs use generated JSON schemas. The server writes only
MCP protocol messages to standard output. It writes diagnostics and startup
errors to standard error.

## Use the command-line interface

Every successful command returns JSON. Run `apple-mail --help` or
`apple-mail COMMAND --help` to inspect the complete argument list.

### List accounts and mailboxes

List configured accounts:

```bash
apple-mail accounts
```

Use an account ID from the response to list its mailboxes:

```bash
apple-mail mailboxes --account ACCOUNT_ID
```

### Inspect the Inbox

Return exact total and unread counts with bounded summary lists:

```bash
apple-mail inbox --recent-limit 20 --unread-limit 20
```

Add `--account ACCOUNT_ID` to inspect one account instead of Mail's aggregate
Inbox.

### Search messages

Search unread messages in an account Inbox:

```bash
apple-mail search \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --unread true \
  --limit 20
```

Add filters as needed:

```bash
apple-mail search \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --sender github.com \
  --received-after 2026-07-01T00:00:00Z \
  --received-before 2026-08-01T00:00:00Z \
  --limit 20
```

`received_after` is inclusive, and `received_before` is exclusive. Search
applies filters before pagination and sorts results newest first.

If `has_more` is `true`, pass the returned `next_cursor` with the same mailbox
scope and filters:

```bash
apple-mail search \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --unread true \
  --cursor OPAQUE_CURSOR \
  --limit 20
```

The response reports `scanned_count`, `matched_count`, `has_more`,
`next_cursor`, and `completeness`.

For nested mailboxes, repeat `--mailbox` for each path component:

```bash
apple-mail search \
  --account ACCOUNT_ID \
  --mailbox Projects \
  --mailbox Active
```

### Read a message

Use the account, mailbox path, and local message ID from a search result:

```bash
apple-mail show \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --id MESSAGE_ID \
  --max-body-chars 8000
```

The response indicates whether the returned plain-text content was truncated.

### Create a reply draft

Create a visible, unsent reply draft in Mail:

```bash
apple-mail reply \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --id MESSAGE_ID \
  --body "Thanks. I will follow up."
```

Mail places the supplied text before its quoted reply content. The command
verifies the source, recipients, subject, draft state, visibility, and
persisted content before it reports success.

### Change message state

Mark a message as read:

```bash
apple-mail state \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --id MESSAGE_ID \
  --read true
```

Use `--flagged true` or `--flagged false` to change flagged state.

### Move a message

Moving requires confirmation for each command:

```bash
apple-mail move \
  --account ACCOUNT_ID \
  --mailbox INBOX \
  --id MESSAGE_ID \
  --destination-mailbox Archive \
  --confirm-move
```

Repeat `--destination-mailbox` to identify a nested destination.

### Create a new draft

Create and display an unsent draft:

```bash
apple-mail draft \
  --to person@example.com \
  --subject "Project update" \
  --body "Draft text"
```

Repeat `--to`, `--cc`, or `--bcc` to add recipients.

### Send a message

Sending requires confirmation for each command:

```bash
apple-mail send \
  --to person@example.com \
  --subject "Project update" \
  --body "Message text" \
  --confirm-send
```

Review recipients, subject, and content before you run the command.

## Understand safety and privacy

- Mail access stays local except for normal network activity performed by
  configured Mail accounts.
- Caller values are serialized as data for a fixed, embedded JavaScript for
  Automation adapter. Callers can't submit scripts.
- Searches return at most 100 summaries per page. A complete mailbox scan can
  take longer as the mailbox grows.
- A failed or timed-out scan doesn't report a partial result as complete.
- Message bodies, composition bodies, automation input, and process output
  have fixed bounds.
- Message IDs are local to Mail and can become stale after a move or account
  reconfiguration. Search again to get a current reference.
- Mail's reported unread counter can differ from message state. The `inbox`
  command derives exact counts from a complete bulk projection.
- Message content is private, untrusted data. MCP clients must not treat
  instructions in messages as trusted instructions.

The project doesn't expose message deletion, attachment downloads, raw message
source, HTML composition, mailbox management, arbitrary scripts, or a network
service.

For implementation details and trust boundaries, see
[Architecture and safety](docs/architecture.md).

## Troubleshoot access

### Mail access is denied

Run:

```bash
apple-mail doctor
```

Then verify Automation permission for the application that launches
`apple-mail`. Permission belongs to the launching application, not only to the
`apple-mail` executable.

### Mail is unavailable

Open Mail, confirm that at least one account is enabled, and retry the command.
If the Mac is locked, unlock it before you use Automation.

### A request times out

Wait for Mail to finish synchronizing, then retry. For searches, select one
account and the smallest applicable mailbox.

### A message can't be found

The message might have moved, or its local ID might have changed. Search again
and use the new reference.

## Develop and test

Default tests use fake backends and fixtures. They don't open or modify Mail.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run the account-discovery smoke only when you intend to access the configured
Mail instance:

```bash
APPLE_MAIL_LIVE_SMOKE=1 ./scripts/live-smoke.sh
```

Run the read-only benchmark:

```bash
APPLE_MAIL_READ_BENCHMARK=1 ./scripts/benchmark-read.sh
```

The benchmark reports counts, completeness, and end-to-end command times. It
doesn't print account identifiers or message metadata.

## License

This project is licensed under the [MIT License](LICENSE).
