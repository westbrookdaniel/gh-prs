# gh-prs

Local-first web UI for GitHub pull requests, built with Rust, a custom `smol` HTTP stack, and Askama templates.

## Requirements

- Rust toolchain (`cargo`)
- GitHub CLI (`gh`) installed and on `PATH`
- Authenticated GitHub CLI session (`gh auth login`)
- Repository context available either from:
  - `--repo OWNER/REPO`, or
  - current working directory with a configured Git remote

## Run

```bash
cargo run -- --repo OWNER/REPO --port 3000
```

Optional flags:

- `--repo OWNER/REPO` (preferred explicit repo)
- `--port PORT` (binds to `127.0.0.1:PORT`)
- `--bind 127.0.0.1:PORT` (explicit bind; localhost only)

## Routes

- `GET /` → redirects to `/prs`
- `GET /health` → JSON health/status payload
- `GET /prs` → PR list (up to 100, all states)
- `GET /prs/:number` → PR detail + discussion + review context
- `POST /prs/:number/comment` → submit top-level PR comment
- `POST /prs/:number/review` → submit review (`approve|comment|request_changes`)

## Testing

Run the test suite:

```bash
cargo test
```

Current suite covers CLI command argument generation, JSON parsing, input validation, error mapping, and handler-level flows.

## Manual MVP Checklist

1. Start app with an authenticated repo:
   - `cargo run -- --repo OWNER/REPO --port 3000`
2. Open `http://127.0.0.1:3000/prs`:
   - list renders, rows link to detail pages
3. Open a PR detail page `http://127.0.0.1:3000/prs/<number>`:
   - header, checks, comments, reviews, and review comments render
4. Submit a comment from the form:
   - redirected back with success/error flash message
5. Submit each review action (`approve`, `comment`, `request_changes`):
   - redirected back with success/error flash message
6. Validate error UX:
   - unauthenticated `gh` or missing repo shows actionable error guidance

## Troubleshooting

- `GitHub CLI Missing` → install `gh` and ensure it is on `PATH`
- `GitHub CLI Not Authenticated` → run `gh auth login`
- `Repository Unavailable` → verify repo name and permissions
- `GitHub CLI Timed Out` → retry and verify network access
