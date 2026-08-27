# Development Guide

This guide covers development workflows for the MESA Tool project.

## Setup


## Development
```bash
# Development environment (with hot reload)
make dev          # Build and start development environment
make dev-logs     # View development logs
make dev-down     # Stop development environment
```

Alternatively, use Docker Compose directly:

```bash
# Development (with hot reload)
docker compose -f docker-compose.dev.yml up -d
```

## Architecture Overview

The stack has a single backend: the Poem Rust web server (`backend/src/web_server`),
which also spawns `reference-outstation` and `reference-control-station` as child processes.

| Service | URL | Notes |
|---|---|---|
| Frontend | http://localhost:3000 | Vite / React |
| Backend | http://localhost:8025 (dev mapped port) | Poem web_server |

All `/api/*` requests from the frontend are proxied to the backend.

---

## Frontend Development

### Option 1: Docker Compose Dev (Recommended)

```bash
make dev        # Build and start dev stack (frontend + backend)
make dev-logs   # View logs
make dev-down   # Stop
```

Access at http://localhost:3000.

### Option 2: Local Frontend with Docker Backend

Run the backend via Docker Compose and the Vite dev server locally:

```bash
# Start backend only
docker compose -f docker-compose.dev.yml up -d backend-dev

# In a separate terminal, run the frontend
cd frontend
npm install
npm run dev
```

Vite picks up `BACKEND_HOST` and `BACKEND_PORT` from the environment. Defaults
(`localhost:8000`) target a locally running Poem backend. To target the
compose-mapped port, set `BACKEND_PORT=8025`.

### Option 3: Fully Local

```bash
# Terminal 1: run the backend
cargo run -p web_server

# Terminal 2: run the frontend
cd frontend
npm install
npm run dev
```

### Frontend Styling

The frontend uses four styling mechanisms, in order of preference:

1. **Tailwind utility classes** (`className="..."`) via the `cn(clsx, twMerge)` helper from `src/lib/utils.ts`. Default for layout, spacing, color, and typography.
2. **`src/components/ui/` wrappers** (shadcn-style): Button, Dialog, Checkbox, Tabs, ScrollArea, and others. These wrap Radix primitives from the `radix-ui` umbrella package using CVA variant props and Tailwind. Use the wrapper for any element it already covers.
3. **`@radix-ui/themes` components** (`Box`, `Text`, `Badge`, `IconButton`, `Popover`, ...) for Radix Themes layout primitives or themed components not covered by the `ui/` wrappers. The whole app is wrapped in `<Theme>` (see `src/main.tsx`).
4. **CSS modules** (`ComponentName.module.css` imported as `styles`) for component-specific styles that need pseudo-selectors, animations, or complex state. See `TestRunnerTab.module.css` for the established pattern.

**Inline `style={{ ... }}` is discouraged.** Use one of the four mechanisms above instead.

```tsx
// Prefer: Tailwind via cn helper
import { cn } from "@/lib/utils"
<div className={cn("flex items-center gap-2 px-4", isActive && "bg-accent")} />

// Prefer: shadcn wrapper with variant prop
import { Button } from "@/components/ui/button"
<Button variant="outline" size="sm">Save</Button>

// Prefer: Radix Themes component
import { Badge, Text } from "@radix-ui/themes"
<Badge size="1">xfail</Badge>

// Avoid: inline style
<span style={{ color: "green", fontWeight: "bold" }}>&#10003;</span>
```

Approximately 50 inline `style` occurrences exist today in `TestResultsTree.tsx` and the `SchedulingTab/` subtree. These are known violations to migrate, not patterns to follow.

---

## Backend Development

### Running the backend locally

```bash
cargo run -p web_server
```

The server listens on `PORT` (default `8000`) and reads data from `DATA_DIR`
(default `backend/src/web_server/../../../data` relative to the crate).

### Running the reference stations locally

```bash
make run-reference-stations
```

This builds and starts `reference-outstation` in the background and
`reference-control-station` in the foreground. The outstation is killed
automatically when the control station exits.

---

## Environment Variables

### Frontend (Vite dev server only)

| Variable | Default | Description |
|---|---|---|
| `BACKEND_HOST` | `localhost` | Backend hostname (Docker: `mesa-backend-dev`) |
| `BACKEND_PORT` | `8000` | Backend port (Docker maps to `8025` on host) |

### Backend (Poem web_server)

| Variable | Default | Description |
|---|---|---|
| `PORT` | `8000` | Port the server listens on |
| `DATA_DIR` | `../../../data` (relative to crate) | Path to the data directory |
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

---

## Project Structure

```
backend/
  src/
    common/                  # Shared Rust library (profile, registry, conformance)
    web_server/              # Poem HTTP server - main runtime binary
      src/
        routes/              # API route handlers (health, profiles, scenarios, jobs, enums)
        services/            # Business logic (job service, process manager)
        models/              # Request/response types
    reference_control_station/  # IEEE 1815.2 reference control station
    reference_outstation/       # IEEE 1815.2 reference outstation
    pics_validator/             # PICS spreadsheet -> JSON profile converter
    scenario_generator/         # Generates conformance test scenarios from a profile

frontend/
  src/
    components/              # React UI components
    api/                     # Generated OpenAPI client (hey-api/openapi-ts)
    profile/                 # Profile editing logic
    utils/                   # Utility functions

data/
  profiles/                  # Curated seed profiles (full.json, demo.json, etc.)
  working/                   # User saves (read-write, not committed)
  template/                  # Read-only template profile
  scenarios.json             # Required at startup (loaded by web_server)
```

---

## Testing

### Frontend

```bash
cd frontend
npm run test        # Vitest unit tests
npm run playwright  # Playwright end-to-end tests (requires backend running)
```

Or via Docker:

```bash
make test-frontend
```

### Backend (Rust)

```bash
cargo test --workspace
```

---

## Pre-commit Hooks

Install both hook types so the `pre-commit` (format/lint/`cargo check`) and
`pre-push` (`cargo test`, `vitest`) gates both run locally:

```bash
pre-commit install --install-hooks --hook-type pre-commit --hook-type pre-push
```

A plain `pre-commit install` only wires the `pre-commit` gate. Without the
`--hook-type pre-push` flag, `.git/hooks/pre-push` is never generated, so
the `cargo-test` and `frontend-test` hooks in `.pre-commit-config.yaml`
never run before a `git push`.

---

## Adding Dependencies

### Frontend

```bash
cd frontend
npm install <package>
```

### Rust

Add to the appropriate crate's `Cargo.toml`. If it belongs to multiple crates,
add it to `[workspace.dependencies]` in the root `Cargo.toml` and reference
it with `{ workspace = true }` in each crate.

---

## Generating the OpenAPI TypeScript Client

The Vite development server generates the frontend API client from the live
Poem `/openapi.json` endpoint.

After changing the backend API, start the backend in another terminal and
refresh both the snapshot and generated client:

```bash
curl --fail --silent --show-error \
  http://localhost:8000/openapi.json \
  --output frontend/openapi.json
cd frontend
npm run generate:api
```

---

## Generating JSON Profiles from XLSX

```bash
make gen-pics
```

Builds `pics-validator` and converts every `.xlsx` in `data/profiles/` to `.json`.

---

## Troubleshooting

### Backend healthcheck fails

The dev compose healthcheck polls `http://localhost:8000/api/health` every 5s
with up to 60 retries. On a cold build (`cargo run` compiling the workspace for
the first time), startup can take several minutes. Wait for the health check to
pass before testing.

### Port conflicts

```bash
make dev-down    # Stop the dev stack
```

### Cargo cache issues (Docker)

The `cargo-cache` and `cargo-registry` named volumes persist between restarts.
To clear them:

```bash
docker compose -f docker-compose.dev.yml down -v
```
