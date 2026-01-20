-- Add sync metadata to folders for bidirectional sync

ALTER TABLE folders
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_folders_updated_at ON folders (updated_at);
CREATE INDEX IF NOT EXISTS idx_folders_is_deleted ON folders (is_deleted);
