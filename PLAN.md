# PLAN: Local GitHub PR Web UI (Rust + smol + Askama)

## Philosophy and Design Guidance

- Local-first over cloud-first: rely on the user's existing `gh` CLI auth/session and never re-implement OAuth in MVP.
- Server-rendered by default: no JavaScript, no client-side state machine, just HTML pages and standard form POST flows.
- Simple beats clever: prefer predictable route handlers and plain data mapping over abstractions that hide behavior.
- Fast feedback loops: every milestone should ship a visible, testable vertical slice.
- Failure should be actionable: every error page should clearly say what failed and how to fix it (`gh auth login`, repo missing, permission denied, etc.).
- Secure by default: bind to localhost, sanitize command inputs, avoid shell interpolation, and do not log sensitive content.
- Keep scope strict: MVP is one repo, one list view, one detail view, plus basic comment/review actions.

## MVP Design Goals

- Render a useful PR list quickly with zero filters and no client JavaScript.
- Render a complete-enough PR detail page for day-to-day review triage.
- Support discussion interactions through standard web forms:
  - Leave a top-level PR comment.
  - Submit a review: approve, comment, or request changes.
- Keep operational complexity low:
  - No cache layer.
  - No background sync workers.
  - No webhook ingestion.

## Scope and Constraints

### In Scope (MVP)

- Single-repo experience.
- PR list via `gh pr list --state all -L 100 --json ...`.
- PR detail via `gh pr view --json ...` + extra `gh api` calls for full conversation surfaces.
- HTML pages rendered with Askama.
- POST-redirect-GET form flow for write actions.

### Out of Scope (MVP)

- PR list filters/search/sort controls.
- Multi-repo aggregated dashboard.
- Inline diff comment creation UI.
- Any persistent DB/cache layer.
- OAuth app flow or token storage in this app.

## Technical Approach

### Runtime Architecture

- Single binary process.
- HTTP server built on the existing custom `smol` framework.
- Template rendering with Askama.
- CLI integration layer shells out to `gh` using `std::process::Command` argument arrays.
- No shell string execution.

### Suggested Module Layout

- `src/main.rs`
  - wiring, routes, app startup.
- `src/gh/mod.rs`
  - shared types and error model.
- `src/gh/client.rs`
  - command execution helpers and JSON parsing.
- `src/gh/models.rs`
  - strongly typed structs for list/detail/comments/reviews.
- `src/views/mod.rs`
  - page view models.
- `src/handlers/mod.rs`
  - route handlers for list/detail/form actions.
- `templates/`
  - `layouts/base.html`
  - `pages/pr_list.html`
  - `pages/pr_detail.html`
  - `pages/error.html`

## CLI Command Contract (MVP)

- Auth/preflight:
  - `gh --version`
  - `gh auth status --json hosts`
- Repo resolution:
  - preferred explicit repo from startup arg/config
  - fallback: `gh repo view --json nameWithOwner,url,viewerPermission,defaultBranchRef`
- PR list:
  - `gh pr list -R OWNER/REPO --state all -L 100 --json number,title,state,isDraft,author,createdAt,updatedAt,url,reviewDecision,reviewRequests,comments`
- PR detail core:
  - `gh pr view NUMBER -R OWNER/REPO --json number,title,body,state,isDraft,author,createdAt,updatedAt,url,baseRefName,headRefName,mergeStateStatus,mergeable,reviewDecision,reviewRequests,latestReviews,statusCheckRollup,commits,files,comments`
- PR detail extras:
  - `gh api repos/OWNER/REPO/issues/NUMBER/comments?per_page=100`
  - `gh api repos/OWNER/REPO/pulls/NUMBER/reviews?per_page=100`
  - `gh api repos/OWNER/REPO/pulls/NUMBER/comments?per_page=100`
- Write actions:
  - comment: `gh pr comment NUMBER -R OWNER/REPO --body-file -`
  - review approve: `gh pr review NUMBER -R OWNER/REPO --approve --body-file -`
  - review comment: `gh pr review NUMBER -R OWNER/REPO --comment --body-file -`
  - review request changes: `gh pr review NUMBER -R OWNER/REPO --request-changes --body-file -`

## Routes and Page Contracts

- `GET /`
  - redirect to `/prs`.
- `GET /prs`
  - render list of up to 100 PRs for current repo.
- `GET /prs/:number`
  - render full detail page with discussion and review context.
- `POST /prs/:number/comment`
  - form field: `body`.
- `POST /prs/:number/review`
  - form fields: `event` (`approve|comment|request_changes`), `body`.
- `GET /health`
  - plain text or JSON heartbeat for local checks.

## UI/UX Requirements (No JS)

- PR list page:
  - table-like rows with PR number, title, state/draft, author, review decision, updated time.
  - each row links to PR detail.
  - refresh by browser reload only.
- PR detail page:
  - header with title, number, state, base/head branch, author, timestamps.
  - reviewers panel: requested reviewers + latest reviewer decisions.
  - checks panel: summarize pass/fail/pending from status rollup.
  - PR body section.
  - issue comments section.
  - review summaries section.
  - review comments section (path + line metadata where available).
  - form blocks for comment and review submission.
- Post/Redirect/Get flow:
  - on successful POST, redirect back to detail page with success flash message.
  - on failure, redirect with error flash message.

## Error Handling and Reliability

- Classify errors into user-fixable vs internal:
  - `gh` missing.
  - `gh` not authenticated.
  - repo not found or no permission.
  - PR not found.
  - command timeout.
  - malformed command output.
- Set command timeouts to avoid hanging requests.
- Never panic in request handlers; always render an error page/response.
- Show concise remediation text on error pages.

## Security and Safety

- Listen only on `127.0.0.1`.
- Parse and validate PR numbers and repo identifiers.
- Execute `gh` commands via args, never with shell interpolation.
- Pass comment/review body through stdin (`--body-file -`).
- Avoid logging full comment/review body.

## Testing Strategy

- Unit tests:
  - command builder correctness (args and flags).
  - JSON parsing into typed models.
  - form input validation.
  - error mapping (stderr/exit code -> app error type).
- Integration tests:
  - handler-level tests for list/detail pages.
  - POST action tests with redirect + flash behavior.
  - stubbed `gh` command execution path.
- Manual verification:
  - real repo smoke test with authenticated local `gh`.

---

## Milestones and Testable Slices

### [x] Milestone 0: Planning and Scope Lock

Goal: lock MVP scope and execution plan.

- [x] Slice 0.1: write this `PLAN.md` with constraints, philosophy, milestones.
- [x] Slice 0.2: align on no-JS SSR approach and `gh` CLI-only backend.

Exit criteria:

- [x] Plan approved and ready for implementation.

### [ ] Milestone 1: App Foundation and Preconditions

Goal: app starts cleanly and verifies local environment.

- [ ] Slice 1.1: add startup preflight (`gh --version`, `gh auth status --json hosts`).
- [ ] Slice 1.2: implement repo resolution (explicit repo arg first, fallback to current repo).
- [ ] Slice 1.3: add shared app state struct (repo, host context, startup diagnostics).
- [ ] Slice 1.4: add `GET /health` and root redirect to `/prs`.

Test checks:

- [ ] Missing `gh` yields clear startup/runtime error page.
- [ ] Unauthenticated `gh` yields actionable message.
- [ ] `cargo test` passes.

Exit criteria:

- [ ] App can boot into a predictable state and explain setup failures.

### [ ] Milestone 2: `gh` Client Layer and Typed Models

Goal: centralize all CLI interactions with typed outputs.

- [ ] Slice 2.1: create command executor with timeout and stdin support.
- [ ] Slice 2.2: implement typed parsers for PR list JSON fields.
- [ ] Slice 2.3: implement typed parsers for PR detail + comments/reviews/review-comments.
- [ ] Slice 2.4: implement write methods for comment/review actions.
- [ ] Slice 2.5: unify error enum for command failure, parse failure, timeout, not found.

Test checks:

- [ ] Unit tests for arg generation and parse paths.
- [ ] Unit tests for error mapping and timeout behavior.
- [ ] `cargo test` passes.

Exit criteria:

- [ ] Handlers can call a stable, typed API instead of raw command strings.

### [ ] Milestone 3: PR List Page (No Filters)

Goal: ship the first useful UI page.

- [ ] Slice 3.1: add Askama template `templates/pages/pr_list.html`.
- [ ] Slice 3.2: add `GET /prs` handler using `gh pr list --state all -L 100`.
- [ ] Slice 3.3: render rows with number/title/state/author/review decision/updated time.
- [ ] Slice 3.4: add link from each row to `/prs/:number`.

Test checks:

- [ ] Handler test for successful render path.
- [ ] Handler test for command failure path.
- [ ] Manual check: list renders in real repo with 0..100 PRs.

Exit criteria:

- [ ] `/prs` is usable as the default home screen.

### [ ] Milestone 4: PR Detail Read View

Goal: render full detail context for one PR.

- [ ] Slice 4.1: add Askama template `templates/pages/pr_detail.html`.
- [ ] Slice 4.2: add `GET /prs/:number` handler.
- [ ] Slice 4.3: aggregate core PR data + issue comments + reviews + review comments.
- [ ] Slice 4.4: render reviewers (requested + latest review decisions).
- [ ] Slice 4.5: render checks/status summary from `statusCheckRollup`.

Test checks:

- [ ] Handler test for valid PR render.
- [ ] Handler test for invalid PR number and not-found behavior.
- [ ] Manual check: detail page shows comments and review context.

Exit criteria:

- [ ] `/prs/:number` is sufficient for read-only review triage.

### [ ] Milestone 5: Write Actions via Standard Forms

Goal: support posting comments and review decisions without JS.

- [ ] Slice 5.1: add comment form section and `POST /prs/:number/comment` handler.
- [ ] Slice 5.2: add review form (approve/comment/request changes) and handler.
- [ ] Slice 5.3: send bodies via stdin to `--body-file -`.
- [ ] Slice 5.4: implement Post/Redirect/Get with flash message support.

Test checks:

- [ ] Unit tests for form validation.
- [ ] Handler tests for success redirect and failure redirect.
- [ ] Manual check: submit each review action against a test PR.

Exit criteria:

- [ ] User can participate in PR discussion and submit review outcomes from the UI.

### [ ] Milestone 6: Error UX and Hardening

Goal: make failures understandable and safe.

- [ ] Slice 6.1: add generic error template and structured user-facing messages.
- [ ] Slice 6.2: sanitize all user-provided route/form inputs.
- [ ] Slice 6.3: improve logging with request id + command class + duration.
- [ ] Slice 6.4: ensure localhost-only bind in startup config.

Test checks:

- [ ] Tests for malformed input handling.
- [ ] Tests for timeout and CLI failure propagation.
- [ ] Manual check: intentional auth/repo failures show actionable guidance.

Exit criteria:

- [ ] Common operational failures are easy to diagnose and recover from.

### [ ] Milestone 7: Ship Readme and MVP Exit Validation

Goal: make the project runnable by others.

- [ ] Slice 7.1: document startup requirements (`gh`, auth, repo context).
- [ ] Slice 7.2: add usage examples and route overview.
- [ ] Slice 7.3: add manual test checklist for final MVP validation.

Test checks:

- [ ] Good test coverage numbers.
- [ ] Fresh local run from docs succeeds.
- [ ] Final smoke test: list -> detail -> comment/review flow works end-to-end.

Exit criteria:

- [ ] MVP is documented, testable, and ready for iterative expansion.

---

## MVP Done Checklist

- [ ] App boots locally with clear setup diagnostics.
- [ ] `/prs` renders up to 100 PRs with no filters.
- [ ] `/prs/:number` renders detail, comments, reviews, reviewers, checks.
- [ ] Comment submission works via standard POST form.
- [ ] Review submission (approve/comment/request changes) works via standard POST form.
- [ ] Error states are actionable and non-crashing.
- [ ] Core tests pass and manual smoke test passes.
