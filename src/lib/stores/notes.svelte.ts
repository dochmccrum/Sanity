import type { Note, NoteSummary, NoteMetadataUpdate } from '$lib/types/note';
import { getNoteRepository } from '$lib/api/adapterContext';
import { getYjsDocManager, uint8ArrayToBase64, type YjsDocManager } from '$lib/sync/YjsDocManager';
import { getWebSocketSyncProvider, type WebSocketSyncProvider } from '$lib/sync/WebSocketSyncProvider';
import { browser } from '$app/environment';
import * as Y from 'yjs';

export function createNotesStore() {
  const repo = getNoteRepository();
  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
  let notes = $state<NoteSummary[]>([]);
  let selectedNote = $state<Note | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  
  // CRDT document manager
  let docManager: YjsDocManager | null = null;
  let wsProvider: WebSocketSyncProvider | null = null;
  let wsServerUrl: string | null = null;
  const contentSnapshotTimers = new Map<string, ReturnType<typeof setTimeout>>();

  // Initialize the Yjs document manager
  function initDocManager(): YjsDocManager {
    if (!docManager) {
      docManager = getYjsDocManager({
        onLocalUpdate: async (noteId, update) => {
          // Persist CRDT state when local changes occur
          if (repo.saveCrdtState) {
            try {
              const doc = docManager!.getDoc(noteId);
              const state = Y.encodeStateAsUpdate(doc);
              const stateVector = Y.encodeStateVector(doc);
              console.log(`CRDT onLocalUpdate: Saving state for ${noteId.slice(0,8)}, size: ${state.length} bytes`);
              await repo.saveCrdtState(noteId, state, stateVector);
            } catch (err) {
              console.error('Failed to persist CRDT state:', err);
            }
          }

          // Notify auto-sync scheduler (Tauri only)
          if (typeof window !== 'undefined' && (window as any).__TAURI__) {
            window.dispatchEvent(new CustomEvent('beck:local-change'));
          }

          if (wsProvider?.isConnected()) {
            wsProvider.pushUpdate(noteId, update);
          }
        },
        onContentChange: (noteId, content) => {
          // Update in-memory note content snapshot (for previews/search)
          const nowIso = new Date().toISOString();

          const noteIndex = notes.findIndex((n) => n.id === noteId);
          if (noteIndex !== -1) {
            notes[noteIndex] = {
              ...notes[noteIndex],
              content,
              updated_at: nowIso,
            };
          }

          if (selectedNote?.id === noteId) {
            selectedNote = {
              ...selectedNote,
              content,
              updated_at: nowIso,
            };
          }

          // Best-effort title extraction from content
          const titleMatch = content.match(/<h1[^>]*>([^<]*)<\/h1>/);
          if (titleMatch) {
            const newTitle = titleMatch[1] || 'Untitled Note';
            const currentTitle =
              (selectedNote?.id === noteId ? selectedNote.title : notes.find((n) => n.id === noteId)?.title) ??
              'Untitled Note';
            if (newTitle && newTitle !== currentTitle) {
              void updateNoteMetadata(noteId, { title: newTitle });
            }
          }

          // Persist a debounced HTML snapshot to the notes table so previews survive restarts.
          // This avoids calling notesStore.updateNote(), which is wrapped by legacy auto-sync in +page.svelte.
          const existingTimer = contentSnapshotTimers.get(noteId);
          if (existingTimer) clearTimeout(existingTimer);
          contentSnapshotTimers.set(
            noteId,
            setTimeout(async () => {
              try {
                const noteToPersist = (selectedNote?.id === noteId ? selectedNote : notes.find((n) => n.id === noteId)) ?? null;
                if (!noteToPersist) return;

                const saved = await repo.saveNote({
                  ...noteToPersist,
                  content,
                  updated_at: new Date().toISOString(),
                });

                const idx = notes.findIndex((n) => n.id === noteId);
                if (idx !== -1) notes[idx] = saved;
                if (selectedNote?.id === noteId) selectedNote = saved;
              } catch (err) {
                console.warn('Failed to persist CRDT content snapshot:', err);
              }
            }, 1500)
          );
        },
      });
    }
    return docManager;
  }

  async function loadNotes(folderId?: string, uncategorisedOnly?: boolean) {
    loading = true;
    error = null;
    try {
      // For uncategorised view, pass null explicitly to get only notes without folder
      // For all notes view (no folderId), pass undefined to get everything
      // For specific folder, pass the folderId
      const filterParam = uncategorisedOnly ? null : folderId;
      const all = await repo.listNotes(filterParam);
      notes = all;
      // Skip sync here - just subscribe to updates. Sync is triggered on connection.
      ensureWebSocketSubscriptions(undefined, true);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load notes';
      console.error('Error loading notes:', err);
    } finally {
      loading = false;
    }
  }

  async function createNote(folderId?: string): Promise<Note | null> {
    error = null;
    try {
      const newNote = await repo.saveNote({
        title: 'Untitled Note',
        content: '',
        folder_id: folderId ?? null,
        is_canvas: false,
        is_deleted: false,
      });
      
      // Initialize CRDT document for the new note
      const manager = initDocManager();
      manager.getDoc(newNote.id);
      
      notes = [newNote, ...notes];
      selectedNote = newNote;
      // Just subscribe to updates, the CRDT will sync on edit
      ensureWebSocketSubscriptions([newNote.id], true);
      return newNote;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to create note';
      console.error('Error creating note:', err);
      return null;
    }
  }

  async function updateNote(note: Note) {
    error = null;
    try {
      await repo.saveNote(note);
      // Update the local copy
      const index = notes.findIndex((n) => n.id === note.id);
      if (index !== -1) {
        notes[index] = note;
      }
      if (selectedNote?.id === note.id) {
        selectedNote = note;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to update note';
      console.error('Error updating note:', err);
    }
  }

  /**
   * Update only metadata fields (not content - that's handled by CRDT)
   */
  async function updateNoteMetadata(noteId: string, updates: Partial<Pick<Note, 'title' | 'folder_id' | 'is_deleted' | 'is_canvas'>>) {
    const note = notes.find(n => n.id === noteId) ?? selectedNote;
    if (!note) return;

    const updated = {
      ...note,
      ...updates,
      updated_at: new Date().toISOString(),
    };

    await updateNote(updated);
  }

  async function deleteNote(id: string) {
    error = null;
    try {
      await repo.deleteNote(id);
      notes = notes.filter((n) => n.id !== id);
      
      // Cleanup CRDT document
      if (docManager) {
        docManager.destroyDoc(id);
      }
      
      if (selectedNote?.id === id) {
        selectedNote = null;
      }
      wsProvider?.unsubscribeFromNote(id);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to delete note';
      console.error('Error deleting note:', err);
    }
  }

  async function moveNote(noteId: string, folderId: string | null) {
    try {
      const updated = await repo.moveNote(noteId, folderId);

      const noteIndex = notes.findIndex((n) => n.id === noteId);
      if (noteIndex !== -1) {
        notes[noteIndex] = updated;
      }
      if (selectedNote?.id === noteId) {
        selectedNote = updated;
      }
    } catch (err) {
      console.error('Failed to move note', err);
    }
  }

  async function selectNote(note: NoteSummary | null) {
    if (!note) {
      selectedNote = null;
      return;
    }
    
    // If we have a full note already selected, checking id
    if (selectedNote?.id === note.id) {
      return;
    }
    
    // If the passed object is already a full Note (has content), use it
    if ('content' in note && typeof note.content === 'string') {
      selectedNote = note as Note;
      await initializeNoteDocument(selectedNote);
      return;
    }

    // Otherwise, fetch full details
    try {
      const fullNote = await repo.getNote(note.id);
      if (fullNote) {
        selectedNote = fullNote;
        await initializeNoteDocument(fullNote);
      }
    } catch(e) {
      console.error("Failed to fetch full note", e);
    }
  }

  /**
   * Initialize the CRDT document for a note
   * Loads existing state from database or initializes with current content
   */
  async function initializeNoteDocument(note: Note) {
    const manager = initDocManager();
    
    // Try to load existing CRDT state
    if (repo.getCrdtState) {
      try {
        const existingState = await repo.getCrdtState(note.id);
        if (existingState) {
          manager.loadState(note.id, existingState.ydoc_state);
          return;
        }
      } catch (err) {
        console.warn('Failed to load CRDT state, initializing fresh:', err);
      }
    }
    
    // No existing state, initialize with current content
    manager.initializeWithContent(note.id, note.content);
    // Just subscribe, don't trigger a full sync
    ensureWebSocketSubscriptions([note.id], true);
  }

  /**
   * Get the Yjs document for a note (for editor binding)
   */
  function getYjsDoc(noteId: string): Y.Doc {
    const manager = initDocManager();
    return manager.getDoc(noteId);
  }

  /**
   * Get the Yjs XmlFragment for editor binding
   */
  function getYjsFragment(noteId: string): Y.XmlFragment {
    const manager = initDocManager();
    return manager.getXmlFragment(noteId);
  }

  // Flag to prevent sync loops
  let isSyncing = false;
  let lastSyncTime = 0;
  const SYNC_DEBOUNCE_MS = 2000; // Minimum time between syncs

  /**
   * Initialize WebSocket sync provider for real-time CRDT sync
   */
  function initWebSocketSync(serverUrl: string, getToken: () => string | null): void {
    if (!browser) return;
    const normalizedUrl = serverUrl.replace(/^http/, 'ws') + '/api/ws';
    if (wsProvider && wsServerUrl === normalizedUrl) return; // Already initialized
    if (wsProvider) {
      wsProvider.disconnect();
      wsProvider = null;
    }
    wsServerUrl = normalizedUrl;

    try {
      wsProvider = getWebSocketSyncProvider({
        serverUrl: normalizedUrl,
        getAuthToken: getToken,
        onConnectionChange: (state) => {
          console.log('WebSocket sync state:', state);
          if (state === 'connected') {
            // Only sync if not already syncing and debounce time passed
            const now = Date.now();
            if (!isSyncing && (now - lastSyncTime) > SYNC_DEBOUNCE_MS) {
              isSyncing = true;
              lastSyncTime = now;
              ensureWebSocketSubscriptions();
            }
          }
        },
        onSyncComplete: (noteIds) => {
          console.log('Synced notes:', noteIds);
          isSyncing = false;
          // Don't reload notes or re-sync - the CRDT updates are already applied
        },
        onMetadataUpdate: async (metadata) => {
          console.log('Received metadata update via WS:', metadata);
          
          if (metadata.is_deleted) {
            notes = notes.filter(n => n.id !== metadata.id);
            if (selectedNote?.id === metadata.id) {
              selectedNote = null;
            }
            // Persist deletion locally if in Tauri
            if (isTauri) {
              await repo.deleteNote(metadata.id);
            }
            return;
          }

          const updatedNote: Note = {
            id: metadata.id,
            title: metadata.title,
            content: metadata.content,
            folder_id: metadata.folder_id,
            is_deleted: metadata.is_deleted,
            is_canvas: metadata.is_canvas,
            updated_at: metadata.updated_at,
          };

          const existingIndex = notes.findIndex(n => n.id === metadata.id);
          if (existingIndex !== -1) {
            // Update existing if newer
            if (new Date(metadata.updated_at) > new Date(notes[existingIndex].updated_at)) {
              notes[existingIndex] = updatedNote;
              if (selectedNote?.id === metadata.id) {
                selectedNote = updatedNote;
              }
              // Persist locally if in Tauri
              if (isTauri) {
                await repo.saveNote(updatedNote);
              }
            }
          } else {
            // Add new note
            notes = [updatedNote, ...notes];
            // Persist locally if in Tauri
            if (isTauri) {
              await repo.saveNote(updatedNote);
            }
            // Subscribe to its content updates
            wsProvider?.subscribeToNote(metadata.id);
          }
        },
        onSyncError: (err) => {
          console.error('Sync error:', err);
          isSyncing = false;
        },
      });
      wsProvider?.connect();
    } catch (err) {
      console.error('Failed to initialize WebSocket sync:', err);
    }
  }

  function buildMetadataSnapshot(): NoteMetadataUpdate[] {
    const manager = initDocManager();
    return notes.map((note) => {
      const contentSnapshot = manager.hasDoc(note.id)
        ? manager.getTextContent(note.id)
        : (note.content ?? '');

      return {
        id: note.id,
        title: note.title,
        content: contentSnapshot,
        folder_id: note.folder_id,
        is_deleted: note.is_deleted,
        is_canvas: note.is_canvas,
        updated_at: note.updated_at,
      };
    });
  }

  function ensureWebSocketSubscriptions(noteIds?: string[], skipSync: boolean = false): void {
    if (!wsProvider) return;

    const ids = noteIds && noteIds.length > 0 ? noteIds : notes.map((note) => note.id);
    for (const id of ids) {
      wsProvider.subscribeToNote(id);
    }

    // Only request sync during initial connection sync (isSyncing=true)
    // or when explicitly requested (skipSync=false and we have specific notes)
    // This prevents the loop where sync response -> loadNotes -> sync
    if (wsProvider.isConnected() && !skipSync && isSyncing) {
      wsProvider.requestSync(ids, buildMetadataSnapshot());
    }
  }

  /**
   * Trigger manual CRDT sync (for Tauri when server URL is configured)
   * 
   * This function handles bidirectional sync:
   * 1. Pushes local notes (metadata + CRDT state) to the server
   * 2. Pulls new/updated notes from the server
   * 3. Creates local notes for server-only notes
   * 
   * @param lastSync - Timestamp of last successful sync. If provided, only notes modified since then will be pushed.
   */
  async function syncCrdtToServer(serverUrl: string, token: string, lastSync: string | null = null): Promise<void> {
    if (!browser) return;

    try {
      const manager = initDocManager();
      const metadata: NoteMetadataUpdate[] = [];
      const stateVectors: Record<string, string> = {};
      const updates: Record<string, string> = {};

      const allNotes = await repo.listNotes(null);

      // Get all local CRDT states from database (if available)
      const localStates = repo.getAllCrdtStates ? await repo.getAllCrdtStates() : [];
      const localStateMap = new Map(localStates.map(s => [s.note_id, s]));

      // Prepare sync request for ALL local notes
      for (const note of allNotes) {
        // Determine if we should push updates (diffs) for this note
        // 1. If no lastSync (first sync), push everything
        // 2. If locally modified since last sync, push
        // 3. If currently open in editor (might have unsaved changes), push
        const shouldPushUpdate = !lastSync || note.updated_at > lastSync || manager.hasDoc(note.id);

        const contentSnapshot = manager.hasDoc(note.id)
          ? manager.getTextContent(note.id)
          : (note.content ?? '');

        // Add metadata only if modified (saves bandwidth)
        if (shouldPushUpdate) {
            metadata.push({
              id: note.id,
              title: note.title,
              content: contentSnapshot,
              folder_id: note.folder_id,
              is_deleted: note.is_deleted,
              is_canvas: note.is_canvas,
              updated_at: note.updated_at,
            });
        }

        // Check if we have this doc loaded in memory (being actively edited)
        if (manager.hasDoc(note.id)) {
          // Doc is in memory - get current state directly from it
          // This captures ALL changes including what TipTap has edited
          stateVectors[note.id] = manager.getStateVectorAsBase64(note.id);
          
          if (shouldPushUpdate) {
            // Always send the full state to ensure server gets all content
            const fullState = manager.getState(note.id);
            // console.log(`Sync: Note ${note.id.slice(0,8)} has doc in memory, state size: ${fullState.length}`);
            if (fullState.length > 2) { // Empty Y.Doc is 2 bytes
              updates[note.id] = uint8ArrayToBase64(fullState);
            }
          }
        } else if (localStateMap.has(note.id)) {
          // We have stored CRDT state but doc not loaded
          const state = localStateMap.get(note.id)!;
          
          // Optimization: Use stored state vector if available, without loading doc
          if (state.state_vector && state.state_vector.length > 0) {
             stateVectors[note.id] = uint8ArrayToBase64(state.state_vector);
          } else {
             // Fallback: load state to compute vector
             manager.loadState(note.id, state.ydoc_state);
             stateVectors[note.id] = manager.getStateVectorAsBase64(note.id);
          }
          
          if (shouldPushUpdate) {
            // Send the stored state
            if (state.ydoc_state.length > 0) {
              updates[note.id] = uint8ArrayToBase64(state.ydoc_state);
            }
          }
        } else {
          // No CRDT state at all - this note has only plain HTML content
          // Populate the Y.Doc from the HTML content so it can be synced
          if (note.content && note.content.trim()) {
            // console.log(`Sync: Note ${note.id.slice(0,8)} has no CRDT state, populating from HTML content`);
            manager.populateFromHtml(note.id, note.content);
            const fullState = manager.getState(note.id);
            stateVectors[note.id] = manager.getStateVectorAsBase64(note.id);
            
            if (shouldPushUpdate && fullState.length > 2) { // Empty Y.Doc is 2 bytes
              updates[note.id] = uint8ArrayToBase64(fullState);
              // Also save the CRDT state locally for future syncs
              if (repo.saveCrdtState) {
                const stateVector = manager.getStateVector(note.id);
                repo.saveCrdtState(note.id, fullState, stateVector).catch(err => {
                  console.warn('Failed to save populated CRDT state:', err);
                });
              }
            }
          } else {
            // Empty note - just create a fresh doc
            manager.getDoc(note.id);
            stateVectors[note.id] = manager.getStateVectorAsBase64(note.id);
          }
        }
      }

      console.log('Sync request:', {
        noteCount: metadata.length,
        stateVectorCount: Object.keys(stateVectors).length,
        updateCount: Object.keys(updates).length,
        updateSizes: Object.fromEntries(
          Object.entries(updates).map(([k, v]) => [k, v.length])
        ),
      });

      // Send sync request to server
      const res = await fetch(`${serverUrl.replace(/\/+$/, '')}/api/sync/crdt`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          state_vectors: stateVectors,
          updates,
          metadata,
        }),
      });

      if (!res.ok) {
        throw new Error(`CRDT sync failed: ${res.status}`);
      }

      const response = await res.json();
      console.log('Sync response:', {
        updateCount: Object.keys(response.updates).length,
        metadataCount: response.metadata?.length || 0,
      });

      // Track which notes we've processed
      const processedNoteIds = new Set<string>();

      // Build a map of ALL local note IDs for quick lookup (not just the filtered view)
      const localNoteMap = new Map(allNotes.map(n => [n.id, n]));

      // Apply CRDT updates from server
      for (const [noteId, updateBase64] of Object.entries(response.updates)) {
        if (!updateBase64) continue;
        processedNoteIds.add(noteId);
        
        // Apply update to document
        manager.applyUpdateFromBase64(noteId, updateBase64 as string, 'sync');
        
        // Persist the merged CRDT state locally
        if (repo.saveCrdtState) {
          const doc = manager.getDoc(noteId);
          const state = Y.encodeStateAsUpdate(doc);
          const stateVector = Y.encodeStateVector(doc);
          await repo.saveCrdtState(noteId, state, stateVector);
        }
      }

      // Process metadata from server - create new notes or update existing
      for (const serverMeta of response.metadata) {
        const localNote = localNoteMap.get(serverMeta.id);

        if (serverMeta.is_deleted) {
          // Delete locally if server marked it deleted
          await repo.saveNote({
            id: serverMeta.id,
            title: serverMeta.title,
            content: serverMeta.content ?? '',
            folder_id: serverMeta.folder_id,
            is_deleted: true,
            is_canvas: serverMeta.is_canvas,
            updated_at: serverMeta.updated_at,
          });

          notes = notes.filter(n => n.id !== serverMeta.id);
          if (selectedNote?.id === serverMeta.id) {
            selectedNote = null;
          }
          continue;
        }
        
        if (!localNote) {
          // NEW note from server - create it locally
          console.log('Creating new note from server:', serverMeta.id, serverMeta.title);
          
          // Extract content from CRDT if we received an update
          let content = serverMeta.content ?? '';
          if (processedNoteIds.has(serverMeta.id) && manager.hasDoc(serverMeta.id)) {
            content = manager.getTextContent(serverMeta.id);
          }
          
          // Save the new note to local storage
          const newNote = await repo.saveNote({
            id: serverMeta.id,
            title: serverMeta.title,
            content,
            folder_id: serverMeta.folder_id,
            is_deleted: serverMeta.is_deleted,
            is_canvas: serverMeta.is_canvas,
          });
          
          // Add to local notes list
          notes = [newNote, ...notes.filter(n => n.id !== newNote.id)];
        } else if (serverMeta.updated_at > localNote.updated_at) {
          // Server has newer metadata - update local
          const updatedContent = processedNoteIds.has(serverMeta.id) && manager.hasDoc(serverMeta.id)
            ? manager.getTextContent(serverMeta.id)
            : (serverMeta.content ?? localNote.content ?? '');

          const updated = await repo.saveNote({
            id: serverMeta.id,
            title: serverMeta.title,
            content: updatedContent,
            folder_id: serverMeta.folder_id,
            is_deleted: serverMeta.is_deleted,
            is_canvas: serverMeta.is_canvas,
            updated_at: serverMeta.updated_at,
          });

          const index = notes.findIndex((n) => n.id === serverMeta.id);
          if (index !== -1) {
            notes[index] = updated;
          }
          if (selectedNote?.id === serverMeta.id) {
            selectedNote = updated;
          }
        }
      }

      console.log('CRDT sync completed successfully');
    } catch (err) {
      console.error('CRDT sync failed:', err);
      throw err;
    }
  }

  /**
   * Disconnect WebSocket sync
   */
  function disconnectSync(): void {
    if (wsProvider) {
      wsProvider.disconnect();
      wsProvider = null;
    }
    wsServerUrl = null;
  }

  function clearError() {
    error = null;
  }

  return {
    get notes() { return notes; },
    get selectedNote() { return selectedNote; },
    get loading() { return loading; },
    get error() { return error; },
    loadNotes,
    createNote,
    updateNote,
    updateNoteMetadata,
    deleteNote,
    moveNote,
    selectNote,
    clearError,
    // CRDT methods
    getYjsDoc,
    getYjsFragment,
    initializeNoteDocument,
    initWebSocketSync,
    syncCrdtToServer,
    disconnectSync,
  };
}
