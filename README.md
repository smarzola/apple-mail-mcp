# apple-mail-mcp

Rust CLI and stdio MCP server for controlling Apple Mail on macOS.

## Read-only live smoke

Default tests use a fake backend and never access Apple Mail. To explicitly test the real macOS Automation boundary without printing message content or account values:

```bash
APPLE_MAIL_LIVE_SMOKE=1 ./scripts/live-smoke.sh
```

The smoke runs only account discovery, validates the JSON shape, and prints a pass/fail status. The first attempt may trigger an Automation prompt for the terminal or host application. If macOS denies access, enable the controlling application under **System Settings > Privacy & Security > Automation**. If the Mac is locked, Mail is unavailable, or the prompt cannot be answered, the command terminates with an actionable error after the backend timeout.
