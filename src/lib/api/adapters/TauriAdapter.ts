import { 
  getAllNotes, 
  getNote as tauriGetNote, 
  getNotesByFolder, 
  saveNote as tauriSaveNote, 
  deleteNote as tauriDeleteNote, 
  moveNote as tauriMoveNote,
  saveCrdtState as tauriSaveCrdtState,
  getCrdtState as tauriGetCrdtState,
  getAllCrdtStates as tauriGetAllCrdtStates,
} from '../notes';
import type { NoteRepository, CrdtState } from '../NoteRepository';
import type { Note, NoteInput, SyncPayload, SyncResult, CrdtSyncRequest, CrdtSyncResponse, NoteMetadataUpdate } from '../../types/note';
import { getWebSocketSyncProvider, getYjsDocManager, YjsDocManager, WebSocketSyncProvider } from '$lib/sync';

function mapToShared(note: any): Note {
  return {
    id: note.id,
    title: note.title ?? '',
    content: note.content ?? '',
    folder_id: note.folder_id ?? null,
    updated_at: note.updated_at ?? new Date().toISOString(),
    is_deleted: note.is_deleted ?? false,
    is_canvas: note.is_canvas ?? false,
  };
}

export class TauriAdapter implements NoteRepository {
  private yjsDocManager: YjsDocManager;
  private wsSyncProvider: WebSocketSyncProvider | null = null;

  constructor() {
    // We get the global manager but don't attach callbacks here.
    // The notesStore manages the lifecycle of document updates and sync.
    this.yjsDocManager = getYjsDocManager();
  }

  private getProvider(): WebSocketSyncProvider | null {
    if (!this.wsSyncProvider) {
      this.wsSyncProvider = getWebSocketSyncProvider();
    }
    return this.wsSyncProvider;
  }

  async listNotes(folderId?: string | null): Promise<Note[]> {
    const notes = folderId === undefined ? await getAllNotes() : await getNotesByFolder(folderId);
    return notes.map(mapToShared);
  }

  async getNote(id: string): Promise<Note | null> {
    const note = await tauriGetNote(id);
    return note ? mapToShared(note) : null;
  }

  async saveNote(note: NoteInput): Promise<Note> {
    const saved = await tauriSaveNote({
      id: note.id,
      title: note.title,
      content: note.content,
      folder_id: note.folder_id ?? null,
      updated_at: note.updated_at,
      is_deleted: note.is_deleted ?? false,
      is_canvas: note.is_canvas ?? false,
    });
    // After saving a note's metadata, push it via WebSocket
    this.getProvider()?.pushMetadata({ 
      id: saved.id, 
      title: saved.title, 
      content: saved.content, 
      folder_id: saved.folder_id, 
      is_deleted: saved.is_deleted, 
      is_canvas: saved.is_canvas, 
      updated_at: saved.updated_at 
    });
    // Also ensure we're subscribed to it
    this.getProvider()?.subscribeToNote(saved.id);
    return mapToShared(saved);
  }

  async deleteNote(id: string): Promise<boolean> {
    const deleted = await tauriDeleteNote(id);
    if (deleted) {
      this.getProvider()?.pushMetadata({ id, is_deleted: true, updated_at: new Date().toISOString() } as NoteMetadataUpdate);
      this.yjsDocManager.destroyDoc(id); // Clean up Yjs doc
      this.getProvider()?.unsubscribeFromNote(id);
    }
    return deleted;
  }

  async moveNote(id: string, folderId: string | null): Promise<Note> {
    await tauriMoveNote(id, folderId);
    const note = await this.getNote(id);
    if (note) {
      this.getProvider()?.pushMetadata({ 
        id: note.id, 
        title: note.title,
        content: note.content,
        is_deleted: note.is_deleted,
        is_canvas: note.is_canvas,
        folder_id: note.folder_id, 
        updated_at: note.updated_at 
      });
    }
    return note ?? {
      id,
      title: '',
      content: '',
      folder_id: folderId,
      updated_at: new Date().toISOString(),
      is_deleted: false,
      is_canvas: false,
    };
  }

  async sync(_payload: SyncPayload): Promise<SyncResult> {
    // Legacy sync - no-op for Tauri (CRDT handles sync)
    return {
      pulled: [],
      last_sync: new Date().toISOString(),
    };
  }

  // CRDT sync methods
  async saveCrdtState(noteId: string, ydocState: Uint8Array, stateVector: Uint8Array): Promise<CrdtState> {
    // When Tauri backend saves state, ensure the YjsDocManager is also updated
    // This method is called from Tauri commands, so it should also inform the YjsDocManager
    this.yjsDocManager.loadState(noteId, ydocState);
    const result = await tauriSaveCrdtState(noteId, ydocState, stateVector);
    return {
      note_id: result.note_id,
      ydoc_state: new Uint8Array(result.ydoc_state),
      state_vector: new Uint8Array(result.state_vector),
      updated_at: result.updated_at,
    };
  }

  async getCrdtState(noteId: string): Promise<CrdtState | null> {
    // Prioritize YjsDocManager's state if available, otherwise fetch from Tauri backend
    if (this.yjsDocManager.hasDoc(noteId)) {
      const ydocState = this.yjsDocManager.getState(noteId);
      const stateVector = this.yjsDocManager.getStateVector(noteId);
      return {
        note_id: noteId,
        ydoc_state: ydocState,
        state_vector: stateVector,
        updated_at: new Date().toISOString(), // Placeholder, actual updated_at might be different
      };
    }

    const result = await tauriGetCrdtState(noteId);
    if (!result) return null;

    // Load into YjsDocManager once fetched from Tauri backend
    this.yjsDocManager.loadState(noteId, new Uint8Array(result.ydoc_state));

    return {
      note_id: result.note_id,
      ydoc_state: new Uint8Array(result.ydoc_state),
      state_vector: new Uint8Array(result.state_vector),
      updated_at: result.updated_at,
    };
  }

  async getAllCrdtStates(): Promise<CrdtState[]> {
    // This is primarily for initial loading or full sync purposes.
    // The WebSocketSyncProvider will handle pushing and pulling updates.
    // For Tauri, we might want to load all existing CRDT states into YjsDocManager
    // and then initiate a full sync.
    const results = await tauriGetAllCrdtStates();
    results.forEach(r => {
      this.yjsDocManager.loadState(r.note_id, new Uint8Array(r.ydoc_state));
    });

    // After loading local states, request a full sync from the server for all known notes
    const allNoteIds = this.yjsDocManager.getAllDocIds();
    // Assuming we have a way to get metadata for all notes without fetching one by one
    // For now, this part might need further refinement based on how metadata is fetched.
    // A simplified approach is to just send all note IDs and let server figure out diffs.
    this.getProvider()?.requestSync(allNoteIds, []); // Send empty metadata initially

    return results.map(r => ({
      note_id: r.note_id,
      ydoc_state: new Uint8Array(r.ydoc_state),
      state_vector: new Uint8Array(r.state_vector),
      updated_at: r.updated_at,
    }));
  }

  async syncCrdt(request: CrdtSyncRequest): Promise<CrdtSyncResponse> {
    // This method is called when the client wants to initiate a full CRDT sync.
    // Instead of making an HTTP request, we use the WebSocket provider's requestSync.
    // The response will be handled asynchronously by the onSyncComplete callback.

    // The request payload contains state vectors and pending updates from the client.
    // We should ensure these are reflected in the YjsDocManager before requesting sync.
    // The prepareSyncRequest in YjsDocManager already handles gathering this info.

    const allNoteIds = Object.keys(request.state_vectors);
    const metadataUpdates: NoteMetadataUpdate[] = request.metadata.map(m => ({
      id: m.id,
      title: m.title,
      content: m.content,
      folder_id: m.folder_id,
      is_deleted: m.is_deleted,
      is_canvas: m.is_canvas,
      updated_at: m.updated_at,
    }));

    this.getProvider()?.requestSync(allNoteIds, metadataUpdates);

    // Return an empty response as the actual sync result comes via WebSocket callback
    return {
      updates: {},
      metadata: [],
      server_time: new Date().toISOString(),
    };
  }
}
