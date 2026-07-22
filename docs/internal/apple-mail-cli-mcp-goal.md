# Goal: Deliver an Apple Mail CLI and stdio MCP server

Work in `/Users/smarzola/projects/apple-mail-mcp`, using the linked implementation worktree at `/Users/smarzola/Documents/Codex/2026-07-22/if-you-wanted-to-create-a/apple-mail-mcp` while the feature branch is active.

Deliver a macOS-only Rust binary that lets a person or a local MCP client inspect and control Apple Mail through a typed, bounded interface. The human CLI and stdio MCP server must share the same behavior, validation, safety policy, and Apple Mail automation backend. The finished implementation must be verified, merged to the default branch, and pushed to the requested GitHub repository.

Source of truth: the user request and the design agreed in the originating Codex thread.

## Target State

When this goal is complete:

- `apple-mail` provides useful read-only CLI commands for accounts, mailboxes, message search, message detail, and fetching new mail.
- It provides explicit commands for message state changes, mailbox moves, draft creation, and sending, with sending disabled in MCP mode unless the operator opts in.
- `apple-mail mcp` exposes equivalent, schema-described tools over stdio without contaminating protocol stdout with logs.
- Apple Mail is controlled only through fixed, bundled automation code and validated typed inputs; callers cannot execute arbitrary AppleScript or JXA.
- Tests exercise validation, parsing, safety policy, CLI behavior, and MCP stdio operation without reading or mutating a developer's real mailbox by default.
- Operator documentation explains capabilities, limitations, privacy boundaries, macOS Automation permission, MCP configuration, and optional signing/notarization.
- The reviewed result is present on `main` in `https://github.com/smarzola/apple-mail-mcp` and in the durable local checkout.

## Current-State Evidence

Verified before implementation:

- `git log -1 --oneline`: the repository contains only `344858d Initial commit`.
- `git status --short --branch`: the durable checkout is clean on `main`; `feat/apple-mail-cli-mcp` starts at `344858d948ab8c171bb693ebd97ea7d7d11a285a`.
- `git remote -v`: `origin` is `git@github.com:smarzola/apple-mail-mcp.git`.
- `/System/Applications/Mail.app/Contents/Resources/Mail.sdef`: installed Mail exposes accounts, nested mailboxes, messages, recipients, message state, move, compose, send, and fetch operations.
- `rustc --version` and `cargo --version`: Rust and Cargo 1.97.1 are installed.
- `/usr/bin/osascript -e 'return "ready"'`: the system automation runner is available.

Unknowns that may affect implementation details, but not the target state:

- macOS may attribute Automation permission to the terminal or MCP host when the fixed JXA adapter runs through `osascript`; document and verify this manually rather than hiding it behind packaging claims.
- Account and mailbox naming varies by user, so live tests must be opt-in and discovery-driven.

## Constraints And Non-Goals

Follow the private user instructions supplied to Codex and any repository-local `AGENTS.md` added by this goal.

- Keep the product macOS-only and local; do not implement IMAP, SMTP, OAuth, a network server, GUI, background daemon, Mail database access, or Mail extensions.
- Do not expose arbitrary scripts, account passwords, permanent deletion, attachment download, or raw RFC message source.
- Do not sign, notarize, release, or change repository visibility; those are separate distribution decisions.
- Do not read message content in default tests or mutate/send real mail during verification.
- Keep MCP stdout protocol-clean and put diagnostics on stderr.
- Bound result counts and body sizes, validate addresses and selectors, and treat message content as private untrusted data.
- Preserve unrelated user changes and work safely in a dirty worktree.
- Implement the smallest coherent design that fully satisfies this goal. Simple is better than complex, but simple must not mean partial: include required behavior, tests, error handling, permissions guidance, and documentation.

## Authorization And Decisions

This goal authorizes repository inspection, in-scope edits, feature-branch commits, non-destructive verification, creation of the named GitHub repository, pushes, and fast-forward delivery to its default branch. It does not authorize sending email, mutating the user's mailbox during tests, making the repository public, publishing releases, signing/notarizing binaries, destructive git operations, or material scope expansion.

Continue through routine implementation choices using repository evidence. Ask only when an ambiguity materially changes behavior, architecture, compatibility, security, or authorization. Otherwise choose the least-surprising bounded interpretation and record it below. Exhaust safe in-scope alternatives before declaring a blocker.

## Success Criteria

The goal is complete only when:

1. The CLI has documented commands for accounts, mailboxes, search, show, check, state mutation, move, draft creation, and explicitly confirmed sending.
2. Read operations return stable bounded models; mutations resolve an explicit mailbox-scoped message reference and cannot run arbitrary automation code.
3. MCP exposes documented equivalents with generated schemas, structured results, accurate annotations, default-deny sending, clean stdio framing, and actionable errors.
4. The automation adapter uses a fixed bundled script, serialized arguments, serialized calls, timeouts/output limits, and sensitive-detail redaction.
5. Default tests use a fake automation boundary; an opt-in read-only live smoke command is documented separately.
6. README and architecture docs cover installation, CLI/MCP configuration, privacy/security, Mail limitations, Automation permission, and development checks.
7. Every milestone is independently reviewed, checked off with evidence, and committed with a focused Conventional Commit.
8. Final verification and a fresh audit pass, then `main` and `origin/main` contain the delivered commits and the durable checkout is clean.

## Milestones

- [x] Milestone 1: Read-only Mail service and CLI
- [x] Milestone 2: Guarded mutations and message composition
- [ ] Milestone 3: Stdio MCP parity and operator documentation

### Checkpoint Protocol

At each milestone boundary: satisfy acceptance criteria; run verification; freeze writes for adversarial review and repair; mark `[x]` with dated command results; commit implementation, tests, docs, and this prompt with a focused Conventional Commit; report the hash. Never check off or commit a failing milestone.

## Milestone 1: Read-only Mail service and CLI

Why this matters: it establishes the safe automation boundary, bounded reads, and human interface before side effects or protocol integration.

Acceptance criteria:

- The project builds as a macOS-only `apple-mail` binary with concise repository instructions.
- Fixed and fake adapters support account discovery, recursive mailbox discovery, bounded search, message detail, and checking for new mail.
- CLI help and errors are actionable; output is stable JSON.
- Tests cover parsing, bounds, selectors, automation failures, and CLI reads without touching Mail.
- An explicitly invoked live read-only smoke can exercise account discovery without printing message content.

Likely touchpoints: `Cargo.toml`, `src/`, `scripts/`, `tests/`, `AGENTS.md`.

Verification:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --test cli_read
```

Status: Complete.

- 2026-07-22: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (4 passed), `cargo test --test cli_read` (2 passed), and `git diff --check` passed. The live `accounts` smoke reached the macOS Automation boundary but waited for more than 20 seconds, as did a direct read-only `osascript` account count; no account values were printed. This is recorded as local permission/session state, not substituted for default automated coverage.
- 2026-07-22 review repair: embedded the fixed script into the binary, changed subprocess capture to enforce byte ceilings while reading and terminate on overflow/timeout, avoided materializing the full message collection, made body truncation Unicode-scalar-safe, classified public automation errors without raw details, completed every read-path fake and CLI test, and added the gated `scripts/live-smoke.sh` procedure. Narrow verification now passes with 13 library tests and 4 CLI integration tests; the unset smoke gate exits 2 before Mail access.
- 2026-07-22 checkpoint: the retained reviewer reported no blocking findings after one repair round. Final `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (13 passed), `cargo test --test cli_read` (4 passed), and `git diff --check` passed.

## Milestone 2: Guarded mutations and message composition

Why this matters: it completes useful Mail control while keeping irreversible external effects deliberate and testable.

Acceptance criteria:

- The service and CLI mark read/unread, flag/unflag, move a mailbox-scoped message, create a visible draft, and send only with explicit confirmation.
- MCP send authorization is represented as reusable policy rather than CLI-only behavior.
- Address, recipient-count, body-size, mailbox, and message-reference validation fail before automation.
- Tests prove default-deny sending, confirmation, selector scoping, request serialization, and error redaction using the fake adapter.
- Verification does not mutate or send real mail.

Likely touchpoints: `src/model.rs`, `src/service.rs`, `src/cli.rs`, `scripts/mail.js`, `tests/`.

Verification:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test mutation
cargo test --test cli_write
```

Status: Complete.

- 2026-07-22: Starting commit `dd584b7`. Added typed state, move, draft, and send operations across models, service, fixed automation dispatch, and CLI. Service policy denies sending by default; CLI construction enables it but every send still requires `--confirm-send`. Validation covers mailbox scope, differing move destinations, address shape, recipient count, subject/body bounds, control characters, and NUL. Verification uses only fake-backed mutation tests and embedded non-Mail automation self-tests; no real mailbox was mutated and no message was sent.
- 2026-07-22 review repair: mutations now require account-scoped source references; move destinations inherit and normalize that account before same-mailbox checks, and mailbox paths have depth/total-size caps. Move results are truthful acknowledgements without synthetic post-move IDs. State results are compared with requested values, draft visibility is read back from Mail, and every state/draft/move confirmation invariant has backend-level regression coverage. Serialized automation input now has a hard byte ceiling.
- 2026-07-22 re-review repair: deriving an account for an unscoped search result now preserves the complete nested fallback mailbox path. An embedded JXA regression proves `Projects/Customer` round-trips with the derived account. Serialized input tests now cover the exact byte ceiling and the first rejected byte.
- 2026-07-22 checkpoint: the retained reviewer reported no blocking findings after two repair rounds. Final `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test mutation` (2 passed), `cargo test --test cli_write` (4 passed), `cargo test --lib` (24 passed), and `git diff --check` passed.

## Milestone 3: Stdio MCP parity and operator documentation

Why this matters: it delivers the requested agent interface and understandable operation without weakening CLI safety.

Acceptance criteria:

- `apple-mail mcp` serves supported operations with generated schemas, structured content, accurate annotations, and protocol-clean stdout.
- Send tools are unavailable or rejected unless MCP starts with an explicit allow-send option.
- An automated stdio test initializes MCP, lists tools, calls a fake-backed read tool, and shuts down cleanly.
- README and architecture docs cover CLI, MCP config, permissions, safety, known Mail/JXA constraints, development, and optional distribution.
- Full regression checks pass.

Likely touchpoints: `src/mcp.rs`, `src/main.rs`, `tests/mcp_stdio.rs`, `README.md`, `docs/architecture.md`.

Verification:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- --help
cargo run -- mcp --help
```

Status: Not started.

## Final Verification

Run from the implementation worktree:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- --help
cargo run -- mcp --help
git diff --check
```

Then verify the durable checkout and remote default branch after fast-forward delivery. A checkbox or commit is not proof that a command passed.

## Decision And Status Notes

- 2026-07-22: Chose a private repository because public publication was not explicitly authorized.
- 2026-07-22: Chose one binary and a fixed JXA-over-`osascript` adapter for the first complete release; a service boundary permits a future backend without adding it now.
- 2026-07-22: Chose JSON as the CLI's initial stable output instead of parallel renderers.
- 2026-07-22: Added explicit stdout piping after a live smoke exposed that `tokio::process::Command::spawn` otherwise inherited child output; a regression test now proves fixed automation output is captured.

## Resume Protocol

On resume, read this file, repository instructions, `git status`, status notes, and recent commits. Verify completed checkpoints and continue from the first unchecked milestone without redoing finished work. Do not weaken target state or criteria silently.

## Final Report

Lead with `Achieved` or `Not achieved`, then report target state, milestone commits, files changed, exact verification, reviewer rounds, remote/default-branch state, and residual risks.
