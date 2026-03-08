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

## Jaeger Tracing

Run Jaeger locally with Docker:

```bash
docker run --rm --name jaeger \
  -p 16686:16686 \
  -p 4317:4317 \
  -p 4318:4318 \
  jaegertracing/all-in-one:1.76.0
```

Start the app with OTLP export pointed at Jaeger:

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318 \
OTEL_SERVICE_NAME=gh-prs \
cargo run -- --port 3000
```

Then:

- Open `http://127.0.0.1:16686`
- Search for service `gh-prs`
- Hit routes like `http://127.0.0.1:3000/prs` or `http://127.0.0.1:3000/health`
- Inspect route-based traces such as `GET /prs` and `GET /repos/:owner/:repo/prs/:number`

If traces do not appear:

- confirm the Jaeger container is still running
- confirm the app was started with `OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318`
- reload the app a few times to generate fresh requests

## Cache and Refresh

- Persistent cache store: SQLite (`rusqlite`) at `~/.config/gh-prs/cache.db`
- Override app home/cache directory with `GH_PRS_HOME=/custom/path`
- Page rendering is cache-first:
  - list/detail/changes render cached sections immediately when present
  - when cache is missing, templates render skeleton sections until fresh data arrives
  - append `nocache=1` to any page URL to force a blocking refresh and bypass cached page data
- A small client refresh pass re-requests the current page with `nocache=1` and morphs the returned HTML with vendored Idiomorph
- Write actions (comment/review/merge/state/reviewers) invalidate PR cache keys so the next page refresh fetches fresh data

## Routes

- `GET /` → redirects to `/prs`
- `GET /health` → JSON health/status payload
- `GET /prs` → cross-repo PR search list (up to 100) with filters/sort via query params
- `GET /repos/:owner/:repo/prs/:number` → repo-aware PR detail + discussion + review context
- `GET /repos/:owner/:repo/prs/:number/changes` → PR file diff view
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
