import type { Note, NoteInput, SyncPayload, SyncResult, CrdtSyncRequest, CrdtSyncResponse } from '../types/note';

export interface CrdtState {
  note_id: string;
  ydoc_state: Uint8Array;
  state_vector: Uint8Array;
  updated_at: string;
}

export interface NoteRepository {
  listNotes(folderId?: string | null): Promise<Note[]>;
  getNote(id: string): Promise<Note | null>;
  saveNote(note: NoteInput): Promise<Note>;
  deleteNote(id: string): Promise<boolean>;
  moveNote(id: string, folderId: string | null): Promise<Note>;
  
  // Legacy sync (for backwards compatibility)
  sync(payload: SyncPayload): Promise<SyncResult>;
  
  // CRDT sync methods
  saveCrdtState?(noteId: string, ydocState: Uint8Array, stateVector: Uint8Array): Promise<CrdtState>;
  getCrdtState?(noteId: string): Promise<CrdtState | null>;
  getAllCrdtStates?(): Promise<CrdtState[]>;
  syncCrdt?(request: CrdtSyncRequest): Promise<CrdtSyncResponse>;
}
