export interface Note {
  id: string;
  title: string;
  content: string;
  folder_id: string | null;
  updated_at: string;
  is_deleted: boolean;
  is_canvas: boolean;
}

export type NoteSummary = Note;

export type NoteInput = Omit<Note, 'id' | 'updated_at'> & {
  id?: string;
  updated_at?: string;
};

// Legacy sync (will be deprecated)
export interface SyncPayload {
  since?: string;
  notes: Note[];
}

export interface SyncResult {
  pulled: Note[];
  last_sync: string;
}

// ============================================================================
// CRDT/Yjs Types for Robust Sync
// ============================================================================

/**
 * CRDT document state stored alongside notes
 * The ydoc_state contains the binary Yjs document state
 * The state_vector is used for efficient diff-based sync
 */
export interface CrdtNoteState {
  id: string;
  ydoc_state: Uint8Array;     // Full Yjs document state (Y.encodeStateAsUpdate)
  state_vector: Uint8Array;   // State vector for diff calculation (Y.encodeStateVector)
  updated_at: string;
}

/**
 * Sync request sent from client to server
 * Contains state vectors for notes the client knows about
 */
export interface CrdtSyncRequest {
  /** Map of note_id -> base64-encoded state vector */
  state_vectors: Record<string, string>;
  /** Updates to push to server: note_id -> base64-encoded update */
  updates: Record<string, string>;
  /** Note metadata updates (title, folder, etc.) */
  metadata: NoteMetadataUpdate[];
}

/**
 * Metadata-only update for a note (non-CRDT fields)
 */
export interface NoteMetadataUpdate {
  id: string;
  title: string;
  content: string;
  folder_id: string | null;
  is_deleted: boolean;
  is_canvas: boolean;
  updated_at: string;
}

/**
 * Sync response from server to client
 * Contains only the diff updates needed by the client
 */
export interface CrdtSyncResponse {
  /** Updates for each note: note_id -> base64-encoded update diff */
  updates: Record<string, string>;
  /** Full metadata for notes that changed or are new to client */
  metadata: NoteMetadataUpdate[];
  /** Server timestamp for this sync */
  server_time: string;
}

/**
 * WebSocket message types for real-time sync
 */
export type WsMessageType = 
  | 'sync_request'
  | 'sync_response'
  | 'update'
  | 'note_metadata'
  | 'awareness'
  | 'subscribe'
  | 'unsubscribe';

export interface WsMessage {
  type: WsMessageType;
  note_id?: string;
  payload: string; // base64-encoded binary data
}

/**
 * Connection state for WebSocket sync
 */
export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'syncing';

/**
 * Sync status for individual notes
 */
export interface NoteSyncStatus {
  id: string;
  state: 'synced' | 'pending' | 'syncing' | 'conflict';
  lastSyncedAt?: string;
  pendingUpdates?: number;
}
