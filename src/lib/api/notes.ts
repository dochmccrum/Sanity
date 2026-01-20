/**
 * TypeScript bindings for the Tauri backend commands
 * Auto-generated types to match the Rust structures
 */

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

// ============================================================================
// Types
// ============================================================================

export interface Note {
  id: string;
  title: string;
  content: string;
  folder_id: string | null;
  updated_at: string;
  is_deleted: boolean;
  is_canvas: boolean;
}

export interface NoteSummary {
  id: string;
  title: string;
  folder_id: string | null;
  updated_at: string;
  is_deleted: boolean;
  is_canvas: boolean;
  content?: string; // Optional because UI might expect it, but we can make it optional
}

export interface NoteInput {
  id?: string;
  title: string;
  content: string;
  folder_id?: string | null;
  updated_at?: string;
  is_deleted: boolean;
  is_canvas: boolean;
}

export interface AssetResult {
  id: string;
  uri: string;
  path: string;
}

export interface CommandError {
  message: string;
}

// ============================================================================
// Note API
// ============================================================================

/**
 * Get all notes from the database
 */
export async function getAllNotes(): Promise<NoteSummary[]> {
  return tauriInvoke<NoteSummary[]>('get_all_notes');
}

/**
 * Get a single note by ID
 */
export async function getNote(id: string): Promise<Note | null> {
  return tauriInvoke<Note | null>('get_note', { id });
}

/**
 * Get notes by folder ID (pass undefined/null for root-level notes)
 */
export async function getNotesByFolder(folderId?: string | null): Promise<NoteSummary[]> {
  return tauriInvoke<NoteSummary[]>('get_notes_by_folder', { folder_id: folderId });
}

/**
 * Save a note (create new or update existing)
 * - Omit `id` to create a new note with auto-generated UUID
 * - Include `id` to update an existing note
 */
export async function saveNote(note: NoteInput): Promise<Note> {
  return tauriInvoke<Note>('save_note', { note });
}

/**
 * Delete a note by ID
 * Returns true if a note was deleted, false if not found
 */
export async function deleteNote(id: string): Promise<boolean> {
  return tauriInvoke<boolean>('delete_note', { id });
}

/**
 * Move a note to a different folder
 */
export async function moveNote(id: string, folderId: string | null): Promise<void> {
  return tauriInvoke<void>('move_note', { id, folder_id: folderId });
}

/**
 * Get locally updated notes since a timestamp (RFC3339). Includes deleted notes.
 */
export async function getNotesUpdatedSince(since?: string | null): Promise<Note[]> {
  return tauriInvoke<Note[]>('get_notes_updated_since', { since });
}

/**
 * Apply notes pulled from the remote server.
 */
export async function applySyncNotes(notes: Note[]): Promise<void> {
  return tauriInvoke<void>('apply_sync_notes', { notes });
}

// ============================================================================
// Asset API
// ============================================================================

/**
 * Save an image from base64 data
 * @param base64Data - Base64 encoded image data (with or without data URL prefix)
 * @param fileExtension - File extension (e.g., 'png', 'jpg', 'webp')
 * @returns Asset info including the local URI for display
 */
export async function saveImageAsset(
  base64Data: string,
  fileExtension: string
): Promise<AssetResult> {
  return tauriInvoke<AssetResult>('save_image_asset', { 
    base64_data: base64Data, 
    file_extension: fileExtension 
  });
}

/**
 * Save raw image bytes as an asset
 * @param data - Raw byte array of the image
 * @param fileExtension - File extension (e.g., 'png', 'jpg', 'webp')
 */
export async function saveImageBytes(
  data: number[],
  fileExtension: string
): Promise<AssetResult> {
  return tauriInvoke<AssetResult>('save_image_bytes', { 
    data, 
    file_extension: fileExtension 
  });
}

/**
 * Delete an asset by its ID
 */
export async function deleteAsset(assetId: string): Promise<boolean> {
  return tauriInvoke<boolean>('delete_asset', { asset_id: assetId });
}

/**
 * List all saved assets
 */
export async function listAssets(): Promise<AssetResult[]> {
  return tauriInvoke<AssetResult[]>('list_assets');
}

/**
 * Get the path to the assets directory
 */
export async function getAssetsPath(): Promise<string> {
  return tauriInvoke<string>('get_assets_path');
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Helper to convert a File object to base64 and save as asset
 */
export async function saveFileAsAsset(file: File): Promise<AssetResult> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = async () => {
      try {
        const base64 = reader.result as string;
        const extension = file.name.split('.').pop() || 'png';
        const result = await saveImageAsset(base64, extension);
        resolve(result);
      } catch (error) {
        reject(error);
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/**
 * Create a new empty note
 */
export function createEmptyNote(folderId?: string | null): NoteInput {
  return {
    title: '',
    content: '',
    folder_id: folderId ?? null,
    is_deleted: false,
    is_canvas: false,
  };
}

/**
 * Create a new canvas note
 */
export function createCanvasNote(folderId?: string | null): NoteInput {
  return {
    title: '',
    content: '{}', // Empty canvas JSON
    folder_id: folderId ?? null,
    is_deleted: false,
    is_canvas: true,
  };
}

// ============================================================================
// CRDT Sync API
// ============================================================================

export interface CrdtState {
  note_id: string;
  ydoc_state: number[];      // Uint8Array as number array
  state_vector: number[];    // Uint8Array as number array
  updated_at: string;
}

/**
 * Save CRDT state for a note (Yjs document binary)
 */
export async function saveCrdtState(
  noteId: string,
  ydocState: Uint8Array,
  stateVector: Uint8Array
): Promise<CrdtState> {
  return tauriInvoke<CrdtState>('save_crdt_state', {
    note_id: noteId,
    ydoc_state: Array.from(ydocState),
    state_vector: Array.from(stateVector),
  });
}

/**
 * Get CRDT state for a note
 */
export async function getCrdtState(noteId: string): Promise<CrdtState | null> {
  return tauriInvoke<CrdtState | null>('get_crdt_state', { note_id: noteId });
}

/**
 * Get all CRDT states
 */
export async function getAllCrdtStates(): Promise<CrdtState[]> {
  return tauriInvoke<CrdtState[]>('get_all_crdt_states');
}

/**
 * Get CRDT states for specific notes
 */
export async function getCrdtStatesForNotes(noteIds: string[]): Promise<CrdtState[]> {
  return tauriInvoke<CrdtState[]>('get_crdt_states_for_notes', { note_ids: noteIds });
}

/**
 * Delete CRDT state for a note
 */
export async function deleteCrdtState(noteId: string): Promise<boolean> {
  return tauriInvoke<boolean>('delete_crdt_state', { note_id: noteId });
}

/**
 * Get CRDT states updated since a timestamp
 */
export async function getCrdtStatesUpdatedSince(since?: string | null): Promise<CrdtState[]> {
  return tauriInvoke<CrdtState[]>('get_crdt_states_updated_since', { since });
}

/**
 * Apply a CRDT update from the server
 */
export async function applyCrdtUpdate(noteId: string, update: Uint8Array): Promise<void> {
  return tauriInvoke<void>('apply_crdt_update', {
    note_id: noteId,
    update: Array.from(update),
  });
}
