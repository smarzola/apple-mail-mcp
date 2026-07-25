# Goal: Deliver A Reliable, Safe Apple Mail Product

Work in `/Users/smarzola/projects/apple-mail-mcp`.

Turn the existing MVP into a dependable daily-use Apple Mail CLI and MCP server. Retrieval must be fast and truthful on real mailboxes without private database access, MCP mutations must be opt-in, the highest-value inbox and reply-draft workflows must be first-class typed operations, and the repository must provide diagnostics, measurable performance checks, CI, and releasable macOS packaging.

Source of truth: the product direction agreed in the Codex thread on 2026-07-25 and this prompt.

Starting branch: `main`.
Working branch: `feat/reliable-apple-mail`.
Base commit: `b1d48d126f6d1e68bbcba9fecb2b29682228c5ca`.

## Target State

When this goal is complete:

- Search uses bounded public Mail Automation bulk reads rather than per-message property round trips, sorts newest-first, supports date filtering and stable cursor pagination, and reports counts, continuation, and completeness explicitly.
- Mail-provided unread counts are labeled as reported rather than exact; an inbox snapshot returns exact counts and bounded recent/unread summaries from one typed operation.
- MCP starts read-only. State changes, moves, and drafts require operator write opt-in; moving and sending retain per-call confirmation, and sending additionally requires operator send opt-in.
- A typed reply-draft operation creates a visible unsent native Mail reply, inserts the requested plain-text body above quoted content, and returns verified source, recipients, subject, visibility, and sent state.
- Operators have a privacy-preserving `doctor` command and opt-in read benchmark. The public repository has macOS CI, a tag-driven universal-binary release workflow with checksum plus optional Developer ID signing/notarization, and documented source, binary, Homebrew-tap, and MCP client installation paths.
- Public documentation states exact behavior and privacy tradeoffs without claiming unpublished releases, configured signing credentials, or performance numbers not produced by the checked-in benchmark.

## Current-State Evidence

Verified before this prompt was written:

- `scripts/mail.js:218-237` scans up to 1,000 messages and reads properties per message; `MessageSummaryList` exposes only `messages`, so bounded or incomplete results are silent.
- A live Inbox on 2026-07-25 contained 3,846 messages. `mailbox.unreadCount()` reported 1 while a bulk `readStatus()` projection found 1,112 unread.
- Live `search --unread true --limit 5` took about 9.8 seconds and `--limit 100` exceeded the fixed 20-second timeout. Bulk projection of all eight summary fields for all 3,846 messages completed in about 4.8 seconds; bulk read-status projection completed in 191 ms.
- `src/main.rs:10-17` makes MCP sending opt-in but enables other mutations by default; `MoveMessageRequest` has no per-call confirmation.
- The server exposes nine tools and has no inbox snapshot, reply draft, or diagnostic operation.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and all 33 tests passed before work began.
- The public repository had no GitHub Actions workflows or releases at goal start.

Unknowns that may refine implementation details but not the target state:

- Mail's reply object behavior varies by account provider. Default tests must use fakes and embedded-script fixtures; no live draft or reply may be created during this goal.
- Signing and notarization credentials may not be configured. The workflow must support them safely when secrets are present and fail clearly when a signed release is requested without them; this goal does not authorize creating secrets or publishing a release.

## Constraints And Non-Goals

Follow `AGENTS.md`.

- Keep the product macOS-only and local. Do not access Mail's private databases, request Full Disk Access, add IMAP/SMTP credentials, or introduce a network service.
- Keep CLI and MCP behavior behind the shared typed service. Keep the fixed embedded JXA source and never interpolate caller data into automation source.
- Keep MCP stdout protocol-only; diagnostics belong on stderr.
- Default tests must not inspect, mutate, draft, or send real mail. Live verification is opt-in and read-only.
- Do not add delete, arbitrary bulk mutation, attachment extraction, mailbox CRUD, analytics, HTML composition, or speculative provider abstractions.
- Preserve the existing account, mailbox, message-detail, state, move, draft, and send capabilities. A documented `0.2.0` result-shape and safety-policy change is allowed because the project is pre-1.0.
- Implement the smallest coherent design that completely satisfies this goal. Prefer direct typed additions and existing patterns over new frameworks, dependencies, configuration layers, or generalized abstractions. Simple must not mean partial: include validation, error paths, compatibility notes, tests, and documentation.

## Authorization And Decisions

This goal authorizes repository inspection, in-scope local edits, feature-branch commits, reviewer subagents, and non-destructive verification, including opt-in read-only Mail probes.

It does not authorize mailbox mutations during verification, pushing, opening or merging a pull request, publishing packages or releases, changing repository settings, creating credentials or certificates, or destructive actions. Implement and locally verify publication workflows without triggering them.

Continue through routine implementation choices using repository evidence. Ask only when an ambiguity materially changes public behavior, architecture, compatibility, security, or authorization. Otherwise choose the least-surprising in-scope interpretation and record the decision below.

Before declaring a blocker, exhaust safe in-scope alternatives. If still blocked, preserve the goal state and report the evidence and smallest required external change.

## Success Criteria

The goal is complete only when:

1. Search bulk-projects summary fields, applies all filters before pagination, orders deterministically newest-first, accepts bounded date filters and an opaque validated cursor, and reports total scanned, total matched, `has_more`, `next_cursor`, and completeness.
2. Search no longer has a silent 1,000-message coverage ceiling, and unit/fixture coverage proves aligned bulk columns, filtering, cursor continuation, malformed cursor rejection, output bounds, and deterministic ties.
3. Mailbox counts identify Mail as their source; inbox snapshot returns exact total/unread counts plus independently bounded recent and unread lists without message bodies.
4. MCP defaults to read-only. Write operations fail before the backend unless `--allow-write` is set; send also requires `--allow-send`; move and send require per-call confirmation. CLI help and MCP annotations/descriptions state the policy.
5. Reply-draft validates source/body bounds, uses Mail's native reply command, remains visible and unsent, preserves quoted content below the supplied body, and verifies returned recipients/source/subject/state before acknowledging success.
6. `doctor` reports platform/backend readiness, account count, and elapsed time without account identifiers or message data. The opt-in benchmark emits only counts/timings/completeness and refuses to run without an explicit environment gate.
7. macOS CI runs formatting, strict Clippy, and tests. A tag workflow builds and checks a universal binary, emits SHA-256 checksums, supports optional Developer ID signing/notarization through documented secrets, and packages MCP configuration metadata without publishing during local verification.
8. README and architecture documentation describe the `0.2.0` interfaces, safety modes, diagnostics, benchmark, release artifacts, installation choices, and remaining public-Automation limitations accurately.
9. Every milestone is independently reviewed, checked off with exact verification evidence, and committed with a focused Conventional Commit.
10. Final formatting, strict Clippy, all tests, shell syntax checks, workflow syntax parsing, release build, and opt-in read-only live smoke pass; a fresh final auditor reports no blocking findings.

## Milestones

- [ ] Milestone 1: Fast, truthful retrieval
- [ ] Milestone 2: Safe policies and high-value workflows
- [ ] Milestone 3: Diagnostics and measurable reliability
- [ ] Milestone 4: CI, release packaging, and product documentation

### Checkpoint Protocol

At the end of each milestone:

1. Satisfy its acceptance criteria.
2. Run its verification commands and inspect the results.
3. Freeze implementation writes and obtain a clean adversarial review from the retained reviewer, repairing and re-reviewing as needed.
4. Mark its checkbox `[x]` and add a dated status note with the outcome, exact commands, and results.
5. Commit implementation, tests, docs, and this prompt update together with a focused Conventional Commit.
6. Report the resulting commit hash before beginning the next milestone.

If verification fails, leave the milestone unchecked and do not make its checkpoint commit. Diagnose and repair in-scope defects instead of weakening tests.

## Milestone 1: Fast, Truthful Retrieval

Why this matters:

- The current implementation is incomplete and can time out on an ordinary mailbox even though public Automation bulk reads are materially faster.

Acceptance criteria:

- Success criteria 1-3 are satisfied for search, mailbox count provenance, and inbox snapshot read behavior.
- Existing get-message behavior and bounds remain intact.
- CLI and MCP expose typed object results rather than silent root arrays for new or changed read operations.

Likely touchpoints:

- `src/model.rs`, `src/service.rs`, `src/automation.rs`, `scripts/mail.js`, `src/cli.rs`, `src/mcp.rs`, read tests.

Verification:

```bash
cargo test automation::tests
cargo test service::tests
cargo test --test cli_read
cargo test --test mcp_stdio
```

Status: Not started.

## Milestone 2: Safe Policies And High-Value Workflows

Why this matters:

- Routine mailbox inspection should be safe to approve, while reply drafting should preserve Mail-native threading without granting send authority.

Acceptance criteria:

- Success criteria 4-5 are satisfied.
- Denied write, move, reply-draft, and send calls are proven not to reach the backend.
- Existing state, move, draft, and send behavior remains available with the documented gates.

Likely touchpoints:

- `src/model.rs`, `src/service.rs`, `src/automation.rs`, `scripts/mail.js`, `src/cli.rs`, `src/main.rs`, `src/mcp.rs`, write and MCP tests.

Verification:

```bash
cargo test service::tests
cargo test --test cli_write
cargo test --test mcp_stdio
```

Status: Not started.

## Milestone 3: Diagnostics And Measurable Reliability

Why this matters:

- Automation permissions and latency are host-specific; operators need private diagnostics and repeatable evidence instead of guesswork.

Acceptance criteria:

- Success criterion 6 is satisfied.
- Diagnostic and benchmark outputs contain no account identifiers, addresses, mailbox names, senders, subjects, message IDs, or bodies.
- Default test and verification commands do not access Mail.

Likely touchpoints:

- Typed service/backend diagnostics, CLI/MCP surfaces, `scripts/live-smoke.sh`, a benchmark script, tests, docs.

Verification:

```bash
cargo test doctor
sh -n scripts/live-smoke.sh
sh -n scripts/benchmark-read.sh
```

Status: Not started.

## Milestone 4: CI, Release Packaging, And Product Documentation

Why this matters:

- A reliable local product must be continuously verified and easy to install without overstating unpublished distribution.

Acceptance criteria:

- Success criteria 7-8 are satisfied.
- Workflow files parse as YAML, packaging scripts pass syntax checks, and a local release build succeeds without credentials or external publication.
- Documentation clearly separates available source installation, locally buildable artifacts, and future published/tap installation.

Likely touchpoints:

- `.github/workflows`, release/packaging scripts and metadata, `Cargo.toml`, `README.md`, `docs/architecture.md`.

Verification:

```bash
cargo build --release --locked
sh -n scripts/package-release.sh
ruby -e 'require "yaml"; Dir[".github/workflows/*.yml"].each { |path| YAML.load_file(path) }'
```

Status: Not started.

## Final Verification

Run from `/Users/smarzola/projects/apple-mail-mcp`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
git diff --check main...HEAD
sh -n scripts/live-smoke.sh
sh -n scripts/benchmark-read.sh
sh -n scripts/package-release.sh
ruby -e 'require "yaml"; Dir[".github/workflows/*.yml"].each { |path| YAML.load_file(path) }'
APPLE_MAIL_LIVE_SMOKE=1 ./scripts/live-smoke.sh
```

Do not treat checkboxes or commits as proof. Inspect failures and repair in-scope regressions. The live smoke must remain account-discovery-only and redact account values.

## Decision And Status Notes

- 2026-07-25: Preserve the public Automation-only privacy boundary. Competitor speed obtained through private Mail databases or IMAP credentials is explicitly out of scope.
- 2026-07-25: Treat Mail's mailbox unread property as reported metadata, not exact truth. Exact Inbox counts come from a bulk status projection.
- 2026-07-25: Version the product as `0.2.0` because search result shapes and default MCP mutation policy intentionally change while pre-1.0.
- 2026-07-25: Build and verify release automation locally, but do not push tags, publish artifacts, create a tap repository, or use signing/notarization credentials without separate authorization.

## Resume Protocol

On resume, first read this prompt, `AGENTS.md`, `git status`, the milestone notes, and recent commits. Verify completed checkpoints and continue from the first unchecked milestone without redoing completed work. New evidence may refine implementation details but must not silently weaken the target state or success criteria.

## Final Report

Lead with `Achieved` or `Not achieved`, then report target-state status, milestone commits, files changed, exact verification results, reviewer rounds and disposition, residual risks, and external publication steps that remain unauthorized.
