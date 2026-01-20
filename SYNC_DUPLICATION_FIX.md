# Sync Duplication Fix

## Problem

Notes were duplicating on the server each time it started, and couldn't be deleted properly.

## Root Cause

In [server/src/api/sync_crdt.rs](server/src/api/sync_crdt.rs), the metadata update logic was using:

```sql
UPDATE notes SET 
    title = $2, 
    folder_id = $3, 
    is_deleted = $4, 
    is_canvas = $5,
    updated_at = $6
WHERE id = $1 AND updated_at < $6
```

**The Problem**: This `UPDATE` only works if the note already exists in the `notes` table. However, CRDT states were being stored in the `crdt_states` table without corresponding entries in `notes`. This caused:

1. **Orphaned CRDT states**: CRDT data exists but no note metadata
2. **Duplication on restart**: When the server restarted, sync would try to create notes from CRDT states again
3. **Deletion failures**: Soft-delete flag updates didn't work because the notes didn't exist

## Solution

Changed the metadata update to use `INSERT ... ON CONFLICT`:

```sql
INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
VALUES ($1, $2, '', $3, $4, $5, $6)
ON CONFLICT (id) DO UPDATE SET
    title = EXCLUDED.title,
    folder_id = EXCLUDED.folder_id,
    is_deleted = EXCLUDED.is_deleted,
    is_canvas = EXCLUDED.is_canvas,
    updated_at = EXCLUDED.updated_at
WHERE notes.updated_at < EXCLUDED.updated_at
```

**Why this works**:
- Creates the note if it doesn't exist
- Updates it if it does exist and the new timestamp is newer
- Ensures CRDT states always have corresponding note metadata
- Allows soft-delete to work properly

## Deploying the Fix

### Option 1: Rebuild Docker container (Recommended)

```bash
# Stop current containers
docker compose down

# Rebuild with the fix
docker compose -f docker-compose.yml -f docker-compose.local.yml up --build
```

### Option 2: Rebuild Rust server locally

```bash
cd server
cargo build --release
# Then restart your server
```

## Database Cleanup (If Needed)

If you have duplicate notes in your database, you can clean them up:

```sql
-- Connect to your Postgres database
-- Check for duplicate notes
SELECT id, title, updated_at, is_deleted 
FROM notes 
ORDER BY title, updated_at DESC;

-- Delete duplicates (keep the newest of each)
WITH duplicates AS (
    SELECT id, 
           ROW_NUMBER() OVER (
               PARTITION BY title, content 
               ORDER BY updated_at DESC
           ) as rn
    FROM notes
)
DELETE FROM notes 
WHERE id IN (
    SELECT id FROM duplicates WHERE rn > 1
);

-- Or just mark old duplicates as deleted
WITH duplicates AS (
    SELECT id, 
           ROW_NUMBER() OVER (
               PARTITION BY title, content 
               ORDER BY updated_at DESC
           ) as rn
    FROM notes
)
UPDATE notes 
SET is_deleted = true
WHERE id IN (
    SELECT id FROM duplicates WHERE rn > 1
);
```

## Testing the Fix

1. Start the server with the fix
2. Create a note on a client
3. Sync to server
4. Delete the note
5. Sync again
6. Restart the server
7. Sync from another client
8. **Expected**: Note should stay deleted, no duplicates

## Files Changed

- [server/src/api/sync_crdt.rs](server/src/api/sync_crdt.rs) - Line 172-195
  - Changed `UPDATE` to `INSERT ... ON CONFLICT` for note metadata
