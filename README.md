# gh-prs

Local-first web UI for GitHub pull requests, built with Rust, a custom `smol` HTTP stack, and Askama templates.

## Requirements

- Rust toolchain (`cargo`)
- GitHub CLI (`gh`) installed and on `PATH`
- Authenticated GitHub CLI session (`gh auth login`)
- GitHub account/org access to repositories you want included in search

## Run

```bash
cargo run -- --port 3000
```

Optional flags:

- `--repo OWNER/REPO` (optional startup repo context for metadata/fallback detail routing)
- `--port PORT` (binds to `127.0.0.1:PORT`)
- `--bind 127.0.0.1:PORT` (explicit bind; localhost only)

## Routes

- `GET /` → redirects to `/prs`
- `GET /health` → JSON health/status payload
- `GET /prs` → cross-repo PR search list (up to 100) with filters/sort via query params
- `GET /repos/:owner/:repo/prs/:number` → repo-aware PR detail + discussion + review context
- `POST /repos/:owner/:repo/prs/:number/comment` → submit top-level PR comment
- `POST /repos/:owner/:repo/prs/:number/review` → submit review (`approve|comment|request_changes`)

## Testing

Run the test suite:

```bash
cargo test
```

Current suite covers cross-repo search argument generation, cache behavior, JSON parsing, input validation, error mapping, and handler-level flows.

## Manual MVP Checklist

1. Start app:
   - `cargo run -- --port 3000`
2. Open `http://127.0.0.1:3000/prs`:
   - cross-repo list renders with filters and sorting controls
3. Open a PR detail page from the list (`/repos/:owner/:repo/prs/:number`):
   - header, checks, comments, reviews, and review comments render
4. Submit a comment from the detail form:
   - redirected back with success/error flash message
5. Submit each review action (`approve`, `comment`, `request_changes`):
   - redirected back with success/error flash message
6. Refresh `/prs` twice and confirm cache hit log appears (`[gh] class=pr.search cache=hit`)
7. Validate error UX:
   - unauthenticated `gh` or missing repo shows actionable error guidance

## Troubleshooting

- `GitHub CLI Missing` → install `gh` and ensure it is on `PATH`
- `GitHub CLI Not Authenticated` → run `gh auth login`
- `Repository Unavailable` → verify repo name and permissions
- `GitHub CLI Timed Out` → retry and verify network access
