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

In the Linux app:

1. Open **Settings → Sync**
2. Set **Server URL** to your deployed domain, e.g. `https://notes.example.com`
3. Set **Username** (currently any non-empty username works)
4. Click **Login** (stores a JWT in local storage)
5. Click **Sync now**

What sync does today:
- Syncs notes via `POST /api/sync`
- Uses a local `last_sync` timestamp stored in `localStorage` (`jfnotes_last_sync`)

Current limitation:
- Folder syncing is not implemented yet; outgoing notes are sent with `folder_id = null` to avoid FK issues on the server.
