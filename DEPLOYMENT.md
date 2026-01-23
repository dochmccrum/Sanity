# Deployment & Sync

## Deploy on Coolify (VPS)

This repo ships a single container image that serves both:
- the static web UI at `/`
- the API at `/api/*`

### Option A: Coolify “Docker Compose” app (recommended)

1. In Coolify: **New Resource → Application → Docker Compose**
2. Point it at this repo and select `docker-compose.yml`.
3. Add environment variables (Coolify UI → Environment):
   - `POSTGRES_PASSWORD` (strong password)
   - `JWT_SECRET` (strong random string)
   - (optional) `POSTGRES_USER`, `POSTGRES_DB`, `RUST_LOG`
4. Set the service/port to expose as `server:8080` (Coolify reverse proxy / domain).
5. Enable Auto Deploy on push.

Health check:
- `GET https://<your-domain>/api/health` should return `ok`.

### Notes
- The `db` service stores data in the `db_data` volume.
- For production you generally do **not** need to expose Postgres on `5432` to the public internet.


## Local development (Docker)

From the repo root:

- `docker compose -f docker-compose.yml -f docker-compose.local.yml up --build`

This starts:
- Postgres on `localhost:5432`
- The server API (and static UI, if built into the image) on `http://localhost:8080`

If you see `bash: syntax error near unexpected token '('`, double-check you didn’t paste a Markdown link (e.g. `-f [docker-compose.yml](...)`) into the terminal.

Note: The Docker image **does not** use `src-tauri/`. It only builds the web UI from `src/` and the API from `server/`. Changes in Tauri won’t affect the container. To update the Docker server/UI, rebuild after changing `src/` or `server/`.


## Linux app (Tauri)

### Build locally

From the repo root:

- `npm install`
- `npm run tauri:build`

Artifacts are generated under:
- `src-tauri/target/release/bundle/`

Common outputs on Linux:
- `appimage/*.AppImage`
- `deb/*.deb`

### Install / run

- AppImage: make it executable and run:
  - `chmod +x src-tauri/target/release/bundle/appimage/*.AppImage`
  - `./src-tauri/target/release/bundle/appimage/*.AppImage`

- Debian/Ubuntu package:
  - `sudo dpkg -i src-tauri/target/release/bundle/deb/*.deb`


## Syncing (desktop ↔ server)

### CRDT-Based Sync (New Architecture)

Beck now uses **CRDTs (Conflict-free Replicated Data Types)** via [Yjs](https://yjs.dev/) for robust, conflict-free synchronization between devices.

#### How It Works

1. **Local-First**: All changes happen locally first, then sync to the server
2. **CRDT Documents**: Each note has a Yjs document that tracks all changes as a binary state
3. **Conflict-Free Merging**: When devices sync, Yjs automatically merges changes without data loss
4. **Real-Time Updates**: WebSocket connection broadcasts changes to other connected devices instantly
5. **Offline Support**: Full offline editing with automatic catch-up when reconnected

#### Technical Details

- **Frontend**: TipTap editor with `@tiptap/extension-collaboration` bound to Yjs documents
- **Desktop Storage**: CRDT binary blobs stored in SQLite alongside note metadata
- **Server Storage**: CRDT states in Postgres `crdt_states` table
- **Sync Protocol**: 
  - HTTP `POST /api/sync/crdt` for initial sync and catch-up
  - WebSocket `/api/ws` for real-time updates
- **State Vectors**: Used for efficient diff-based sync (only missing changes are transferred)

#### Comparison to Previous "Last-Write-Wins" Sync

| Aspect | Old (LWW) | New (CRDT) |
|--------|-----------|------------|
| Conflict Handling | Overwrites data silently | Automatically merges all changes |
| Offline Edits | Can lose edits on sync | Never loses edits |
| Real-time Sync | Poll-based | WebSocket push |
| Data Integrity | Medium (clock skew issues) | Highest (mathematically impossible to diverge) |

#### Setup

In the Linux app:

1. Open **Settings → Sync**
2. Set **Server URL** to your deployed domain, e.g. `https://notes.example.com`
3. Set **Username** (currently any non-empty username works)
4. Click **Login** (stores a JWT in local storage)
5. Sync happens automatically via WebSocket, or click **Sync now** for manual sync

#### Server Requirements

Ensure the following migrations have been applied:
- `server/migrations/0003_folders_sync.sql` - folder sync support
- `server/migrations/0004_crdt_states.sql` - CRDT binary blob storage

#### Database Schema

```sql
-- CRDT state table
CREATE TABLE crdt_states (
    note_id UUID PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    ydoc_state BYTEA NOT NULL,      -- Full Yjs document state
    state_vector BYTEA NOT NULL,    -- For efficient diff calculation
    updated_at TIMESTAMPTZ NOT NULL
);
```

### Legacy Sync (Deprecated)

The old REST-based sync is still available for backwards compatibility:

- `POST /api/sync` - note sync
- `POST /api/sync/folders` - folder sync

This uses "last-writer-wins" semantics based on `updated_at` timestamps. New installations should use CRDT sync.
