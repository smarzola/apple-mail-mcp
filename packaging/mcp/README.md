# MCP configuration

Install `apple-mail` on `PATH`, then use the fields in `mcp-server.json` to
configure a stdio MCP server in your client. The packaged metadata intentionally
starts the server in read-only mode.

To permit state changes, moves, mail checks, and drafts, append
`--allow-write`. To also permit confirmed sends, append both `--allow-write`
and `--allow-send`. Moving and sending still require confirmation in each tool
call.
