# Working Agreements

- Keep the product macOS-only and local. Do not access Mail's private databases.
- Keep CLI and MCP behavior behind the shared typed service.
- Never interpolate caller data into automation source or expose arbitrary scripts.
- Keep MCP stdout protocol-only; diagnostics belong on stderr.
- Default tests must not read, mutate, or send real mail.
- Use Conventional Commits. Before publishing, run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.

