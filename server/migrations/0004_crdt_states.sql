-- CRDT state table for Yjs document binary blobs
-- Stores the full Yjs document state for conflict-free sync

CREATE TABLE IF NOT EXISTS crdt_states (
    note_id UUID PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    ydoc_state BYTEA NOT NULL,
    state_vector BYTEA NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_crdt_states_updated_at ON crdt_states (updated_at);

-- Add comment for documentation
COMMENT ON TABLE crdt_states IS 'Stores Yjs CRDT document states for conflict-free sync';
COMMENT ON COLUMN crdt_states.ydoc_state IS 'Full Yjs document state (Y.encodeStateAsUpdate)';
COMMENT ON COLUMN crdt_states.state_vector IS 'State vector for efficient diff sync (Y.encodeStateVector)';
