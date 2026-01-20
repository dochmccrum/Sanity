# CRDT Sync Integration Fixes

## Problem Summary

The CRDT sync infrastructure was built but never connected to the actual app flow:

1. **Content not syncing**: The Yjs documents were being updated locally but never sent to the server
2. **Legacy sync still running**: The old REST-based "Last-Write-Wins" sync was still active, potentially overwriting CRDT changes
3. **WebSocket provider never initialized**: The real-time sync component existed but was never instantiated

## Changes Made

### 1. Integrated WebSocket Sync Provider into Notes Store

**File**: `src/lib/stores/notes.svelte.ts`

Added methods to the notes store:
- `initWebSocketSync(serverUrl, getToken)` - Initializes real-time WebSocket connection
- `syncCrdtToServer(serverUrl, token)` - Manual CRDT sync via HTTP POST to `/api/sync/crdt`
- `disconnectSync()` - Cleanup method

The store now:
- Persists CRDT states to local SQLite when content changes
- Can push all local CRDT states to the server
- Merges incoming CRDT updates from the server
- Updates note metadata (title, folder_id) when server has newer changes

### 2. Replaced Legacy Sync with CRDT Sync

**File**: `src/routes/+page.svelte`

The `handleSyncNow()` function now:
1. Syncs folders first (still using REST `/api/sync/folders`)
2. Calls `notesStore.syncCrdtToServer()` to sync CRDT content
3. Reloads notes to reflect server changes

**Removed**: The old REST-based `/api/sync` endpoint call for notes

### 3. Exported Base64 Helpers

**File**: `src/lib/sync/YjsDocManager.ts`

Exported `uint8ArrayToBase64` and `base64ToUint8Array` functions so the notes store can encode/decode CRDT binary data for HTTP transport.

## How It Works Now

### Desktop (Tauri) Sync Flow

1. **User edits a note** → CollaborativeEditor updates Yjs document
2. **YjsDocManager** triggers `onLocalUpdate` callback
3. **Notes store** saves CRDT state to SQLite via `repo.saveCrdtState()`
4. **Auto-sync** (every 1.5s after changes) calls `handleSyncNow()`
5. **Sync process**:
   - Syncs folders via REST
   - Gets all local CRDT states from SQLite
   - Sends CRDT states + metadata to server `/api/sync/crdt`
   - Server merges CRDT states and returns missing updates
   - Client applies server updates to local Yjs documents
   - Metadata (title, folder) updated if server is newer

### What Gets Synced

**Via CRDT (conflict-free)**:
- Note content (HTML from TipTap editor)
- All edits, even concurrent ones

**Via REST (Last-Write-Wins)**:
- Folders (create, rename, delete)
- Note metadata (title, folder_id, is_deleted, is_canvas)

## Testing Checklist

### Basic Functionality
- [ ] Create a new note - does it appear in the list?
- [ ] Edit note content - does it save?
- [ ] Edit note title - does it update?
- [ ] Move note to folder - does it move?
- [ ] Delete note - does it disappear?

### Folder Operations
- [ ] Create folder - does it appear?
- [ ] Rename folder - does name update?
- [ ] Move note into folder - does it appear in folder?
- [ ] Delete folder - do notes in it get deleted too?
- [ ] Switch between folders - do correct notes show?

### Sync (Tauri → Server)
- [ ] Configure server URL in Settings
- [ ] Login with username
- [ ] Edit a note
- [ ] Click "Sync now" or wait for auto-sync
- [ ] Check server database - is CRDT state present?
- [ ] Check another device - does content appear?

### Sync (Server → Tauri)
- [ ] Edit a note on server/web
- [ ] Sync on desktop
- [ ] Does desktop get the changes?
- [ ] No data loss?

### Conflict Resolution
- [ ] Edit same note on two devices offline
- [ ] Sync both devices
- [ ] Do both sets of changes appear? (CRDT should merge)

## Known Issues & Future Work

### Folders Still Use Legacy Sync
Folders are not yet using CRDT - they use the old REST `/api/sync/folders` endpoint with Last-Write-Wins semantics. This could cause folder conflicts.

**Future enhancement**: Migrate folders to CRDT as well.

### WebSocket Real-Time Sync Not Active
The `WebSocketSyncProvider` is initialized in the notes store but not automatically connected. Currently only HTTP-based manual sync works.

**To enable**:
- Call `notesStore.initWebSocketSync(serverUrl, () => localStorage.getItem('jwt'))` after login
- Subscribe to opened notes via `wsProvider.subscribeToNote(noteId)`

### No Awareness/Presence
The CRDT sync doesn't show remote cursors or active users.

**Future enhancement**: Add y-protocols awareness for collaborative editing UX.

## Debugging

### Check if CRDT state is saved locally

**Tauri SQLite**:
```sql
SELECT note_id, length(ydoc_state), updated_at 
FROM crdt_states 
ORDER BY updated_at DESC;
```

### Check if CRDT state reached server

**Server Postgres**:
```sql
SELECT note_id, length(ydoc_state), updated_at 
FROM crdt_states 
ORDER BY updated_at DESC;
```

### Enable debug logs

Browser console will show:
- "Failed to persist CRDT state:" - Local save failed
- "CRDT sync completed successfully" - Sync succeeded
- "CRDT sync failed:" - Sync failed
- "WebSocket sync state:" - Connection status changes

### Common Problems

**"Cannot find module 'yjs'"**
- Run `npm install` to install dependencies

**"Sync failed: 401"**
- JWT expired or invalid - login again

**"Sync failed: 500"**
- Check server logs
- Ensure migrations ran (`0004_crdt_states.sql`)

**Content not syncing but no errors**
- Check if `saveCrdtState` is being called (console logs)
- Check if auto-sync is triggering
- Verify server URL is set correctly

**Folders broken / notes not appearing**
- Check folder_id foreign key constraints
- Ensure folder sync completed before note sync
- Try reloading notes: `await notesStore.loadNotes()`
