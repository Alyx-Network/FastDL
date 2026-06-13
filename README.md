# FastDL

A high-performance FastDL file server for CS:GO / Source engine games, written in Rust (axum + tokio). Serves custom content (maps, models, materials, sounds) over HTTP with range support and a rule-based access control system.

## How it serves files

Files live under `storage/` and map directly to the request path:

- `http://host:3000/maps/de_dust2.bsp.bz2` → `storage/maps/de_dust2.bsp.bz2`

The server sends files **raw** with `Accept-Ranges: bytes` and full `Range` support. Compressed files (`.bz2`, `.gz`) are delivered as-is with no `Content-Encoding` header, so the Source engine downloads and decompresses them itself — the correct FastDL behaviour. If an exact file is missing, the server transparently falls back to a `.bz2` then `.gz` variant.

## Run locally

```bash
cargo build --release
./target/release/fastdl
```

Place your content under `storage/` (created automatically on first run).

### Port

Resolved in order: `PORT` environment variable → `global.port` in `config.yaml` → `3000`.

## Docker Compose

```bash
docker compose up -d --build
```

This builds the multi-stage `Dockerfile`, exposes port `3000`, and mounts:

- `./storage` → `/app/storage` (persistent content volume)
- `./config.yaml` → `/app/config.yaml` (read-only rules)

## Nixpacks

`nixpacks.toml` builds with the Rust provider and starts `./target/release/fastdl`. Deploy on any nixpacks-based platform (Railway, Coolify, etc.) with a persistent volume mounted at `storage/` and `PORT` set as needed.

## Configuration

`config.yaml` is hot-reloaded on change.

```yaml
global:
  port: 3000
```

### Directory listing

```yaml
directory_listing:
  enabled: true
  allowed_paths:
    - "/"
    - "/public"
```

- `enabled: false` disables listing everywhere.
- Empty `allowed_paths` with `enabled: true` allows all directories.
- A path like `/public` allows `/public` and every subdirectory.

### Rules

Rules are evaluated in order; the first rule whose `conditions` match and whose `action` is decisive wins. Each rule is:

```yaml
rules:
  - name: "admin-area-access"
    description: "optional, free text"
    conditions:
      - field: path
        operator: starts_with
        value: "/admin"
    action: access
    message: "Authentication required"
    status: 401
    access:
      bearer:
        - "change-me-secret-token"
      header:
        - field: header:x-api-key
          operator: equals
          value: "change-me-api-key"
```

**Conditions** — the top-level list is AND (all must match). Any entry may instead be a nested group:

```yaml
conditions:
  - or:
      - { field: userAgent, operator: contains, value: "bot" }
      - { field: userAgent, operator: contains, value: "crawler" }
  - and:
      - { field: path, operator: starts_with, value: "/api" }
      - { field: method, operator: equals, value: "GET" }
```

**Fields:** `path`, `user_agent`, `method`, `ext`, `client_ip` (or `ip`), `peer_ip`, `header:NAME`

- `client_ip` is the visitor IP derived from `cf-connecting-ip` / `x-real-ip` / `x-forwarded-for` (these headers are spoofable; trust them only when a proxy you control sets them).
- `peer_ip` is the raw TCP connection source — it cannot be spoofed by headers. Use it to enforce *where the connection physically comes from*.

**Operators** (case-insensitive, underscores ignored): `equals`, `not_equals`, `starts_with`, `ends_with`, `contains`, `matches` (regex), `in`, `not_in`, `exists`, `not_exists`

For `client_ip` and `peer_ip`, the `equals`/`in`/`not_in` operators accept CIDR ranges (e.g. `10.0.0.0/8`) as well as plain addresses.

### Cloudflare-only access

The shipped config blocks any connection that does not arrive through Cloudflare by checking the connection peer against Cloudflare's published ranges:

```yaml
- name: "cloudflare-only"
  conditions:
    - field: peer_ip
      operator: not_in
      value: [ "173.245.48.0/20", "104.16.0.0/13", ... ]
  action: deny
  status: 403
```

This uses `peer_ip` (not `client_ip`) on purpose: behind Cloudflare, `cf-connecting-ip` carries the *visitor's* IP, so gating on `client_ip` would reject all real Cloudflare traffic. Keep Cloudflare's range list current from <https://www.cloudflare.com/ips/>.

**Actions:**

| Action | Effect |
| ------ | ------ |
| `deny` | Block with `status` (default `403`) and `message`. |
| `allow` | Serve immediately, skipping later rules. |
| `access` | Require the request to satisfy `access` (a valid `bearer` token **or** any `header` condition). If satisfied, evaluation continues; otherwise deny with `status` (default `401`). |

`access.bearer` matches the `Authorization: Bearer <token>` header against the listed tokens. `access.header` is a list of conditions; any one matching grants access.

The extensions `.inc`, `.sp`, `.smx` are always blocked regardless of rules.

## Logging

Structured logging via `tracing`. Control verbosity with `RUST_LOG` (e.g. `RUST_LOG=info`).
