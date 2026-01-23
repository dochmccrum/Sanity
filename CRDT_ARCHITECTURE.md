# CRDT Sync Architecture

This document describes the CRDT-based synchronization architecture for Beck.

## Overview

Beck uses **CRDTs (Conflict-free Replicated Data Types)** via [Yjs](https://yjs.dev/) for robust, conflict-free synchronization between devices. This replaces the previous "Last-Write-Wins" strategy that could lose data during concurrent edits.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              FRONTEND                                    │
│  ┌─────────────────┐    ┌───────────────────┐    ┌──────────────────┐  │
│  │   TipTap +      │    │   YjsDocManager   │    │  WebSocket Sync  │  │
│  │ Collaboration   │◄──►│   (Y.Doc per      │◄──►│    Provider      │  │
│  │   Extension     │    │     note)         │    │                  │  │
│  └─────────────────┘    └───────────────────┘    └────────┬─────────┘  │
│                                   │                        │            │
│                                   ▼                        │            │
│  ┌─────────────────────────────────────────────────────┐  │            │
│  │              Notes Store (Svelte 5)                  │  │            │
│  │  - Note metadata (title, folder_id, etc.)           │  │            │
│  │  - Y.Doc references                                 │  │            │
│  └─────────────────────────────────────────────────────┘  │            │
└─────────────────────────────────────────────────────────────────────────┘
                                   │                        │
         ┌─────────────────────────┼────────────────────────┘
         │                         │
         ▼                         ▼
┌─────────────────┐       ┌────────────────────┐
│  TAURI BACKEND  │       │      SERVER        │
│  ┌───────────┐  │       │  ┌──────────────┐  │
│  │  SQLite   │  │       │  │   Postgres   │  │
│  │           │  │       │  │              │  │
│  │ • notes   │  │       │  │ • notes      │  │
│  │ • crdt_   │  │       │  │ • crdt_      │  │
│  │   states  │  │       │  │   states     │  │
│  └───────────┘  │       │  └──────────────┘  │
└─────────────────┘       │                    │
                          │  ┌──────────────┐  │
                          │  │   SyncHub    │  │
                          │  │  (WebSocket  │  │
                          │  │   Broadcast) │  │
                          │  └──────────────┘  │
                          └────────────────────┘
```

## Component Details

### Frontend

#### YjsDocManager (`src/lib/sync/YjsDocManager.ts`)

Manages Yjs documents for all notes:
- Creates/retrieves Y.Doc for each note
- Handles binary state encoding/decoding
- Tracks pending updates for offline sync
- Provides XmlFragment for TipTap binding

#### WebSocketSyncProvider (`src/lib/sync/WebSocketSyncProvider.ts`)

Handles real-time sync via WebSockets:
- Connects to server WebSocket endpoint
- Subscribes to note updates
- Broadcasts local changes to server
- Applies remote updates to local Y.Docs

#### SyncStore (`src/lib/sync/SyncStore.svelte.ts`)

Svelte 5 reactive store for sync state:
- Tracks connection status
- Manages per-note sync status
- Coordinates between YjsDocManager and WebSocketSyncProvider
- Handles HTTP fallback for initial sync

#### CollaborativeEditor (`src/lib/components/CollaborativeEditor.svelte`)

TipTap editor with Yjs integration:
- Uses `@tiptap/extension-collaboration` for Y.Doc binding
- Content changes automatically go through CRDT
- Supports all existing features (math, images, etc.)

### Tauri Backend

#### Database Changes (`src-tauri/src/database.rs`)

New table for CRDT states:
```sql
CREATE TABLE crdt_states (
    note_id TEXT PRIMARY KEY,
    ydoc_state BLOB NOT NULL,      -- Full Yjs document state
    state_vector BLOB NOT NULL,    -- For efficient diff sync
    updated_at TEXT NOT NULL
);
```

#### Commands (`src-tauri/src/commands.rs`)

New CRDT commands:
- `save_crdt_state` - Persist Y.Doc state
- `get_crdt_state` - Load Y.Doc state
- `get_all_crdt_states` - Load all states
- `apply_crdt_update` - Apply remote update

### Server

#### CRDT Sync Endpoint (`server/src/api/sync_crdt.rs`)

HTTP endpoint for initial/catch-up sync:
- `POST /api/sync/crdt`
- Accepts client state vectors
- Returns only missing updates (diff sync)

#### WebSocket Handler

Real-time sync via WebSockets:
- `GET /api/ws` (upgrade to WebSocket)
- Message types:
  - `subscribe` / `unsubscribe` - Note subscription
  - `update` - Push/receive updates
  - `sync_request` / `sync_response` - Full sync

#### SyncHub (`server/src/api/sync_crdt.rs`)

Broadcasts updates to connected clients:
- Uses tokio broadcast channel
- Filters by subscribed notes

## Sync Flow

### Real-time (Online)

1. User edits note in TipTap
2. TipTap updates Y.Doc via Collaboration extension
3. YjsDocManager captures update, persists to SQLite
4. WebSocketSyncProvider sends update to server
5. Server stores update, broadcasts to other clients
6. Other clients receive update, apply to their Y.Doc

### Offline → Online (Catch-up)

1. User edits offline, changes stored in SQLite
2. User comes online
3. WebSocketSyncProvider reconnects
4. Client sends state vectors for all notes
5. Server calculates diff, sends missing updates
6. Client applies updates, automatic CRDT merge

## Data Model

### Note Table (unchanged)
```typescript
interface Note {
  id: string;
  title: string;
  content: string;        // HTML for backwards compat
  folder_id: string | null;
  updated_at: string;
  is_deleted: boolean;
  is_canvas: boolean;
}
```

### CRDT State Table (new)
```typescript
interface CrdtNoteState {
  note_id: string;
  ydoc_state: Uint8Array;   // Y.encodeStateAsUpdate(doc)
  state_vector: Uint8Array; // Y.encodeStateVector(doc)
  updated_at: string;
}
```

## Migration Strategy

1. **Backwards Compatible**: Existing notes work without CRDT
2. **Lazy Migration**: CRDT state created when note is edited
3. **Fallback**: Legacy sync still works for older clients

## Dependencies Added

### Frontend (package.json)
```json
{
  "@tiptap/extension-collaboration": "^3.15.3",
  "y-indexeddb": "^9.0.12",
  "y-prosemirror": "^1.3.4",
  "y-protocols": "^1.0.6",
  "y-websocket": "^2.1.0",
  "yjs": "^13.6.22"
}
```

### Server (Cargo.toml)
```toml
axum = { features = ["ws"] }
tokio-stream = "0.1"
base64 = "0.22"
futures = "0.3"
dashmap = "6"
```

## Testing

1. **Single Device**: Edit note, verify CRDT state saved
2. **Multi-Device**: Edit same note on two devices, verify merge
3. **Offline**: Edit offline, reconnect, verify sync
4. **Conflict**: Simultaneous edits, verify no data loss

## Future Enhancements

1. **Awareness**: Show remote cursors/presence
2. **History**: Use CRDT for undo/redo history
3. **Selective Sync**: Sync only specific notes/folders
4. **Compression**: Compress CRDT states for efficiency
