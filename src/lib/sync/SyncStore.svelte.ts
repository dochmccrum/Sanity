/**
 * CRDT Sync Store
 * 
 * Svelte 5 reactive store that manages CRDT synchronization state
 * and coordinates between local storage and remote sync.
 */

import { browser } from '$app/environment';
import type { 
  ConnectionState, 
  NoteSyncStatus, 
  NoteMetadataUpdate,
  CrdtSyncRequest,
  CrdtSyncResponse 
} from '$lib/types/note';
import { getYjsDocManager, uint8ArrayToBase64 } from './YjsDocManager';
import { getWebSocketSyncProvider, type SyncProviderOptions } from './WebSocketSyncProvider';

export interface SyncStoreOptions {
  /** Server URL for WebSocket sync */
  wsServerUrl?: string;
  /** HTTP base URL for REST sync fallback */
  httpBaseUrl?: string;
  /** Get auth token */
  getAuthToken: () => string | null;
  /** Persist CRDT state callback */
  persistCrdtState?: (noteId: string, state: Uint8Array) => Promise<void>;
  /** Load CRDT state callback */
  loadCrdtState?: (noteId: string) => Promise<Uint8Array | null>;
}

/**
 * Creates a reactive sync store for managing CRDT synchronization
 */
export function createSyncStore(options: SyncStoreOptions) {
  // Reactive state
  let connectionState = $state<ConnectionState>('disconnected');
  let noteSyncStatuses = $state<Map<string, NoteSyncStatus>>(new Map());
  let lastSyncTime = $state<string | null>(null);
  let syncError = $state<string | null>(null);
  let pendingMetadataUpdates = $state<NoteMetadataUpdate[]>([]);

  const docManager = getYjsDocManager({
    onLocalUpdate: (noteId, update) => {
      // Mark note as having pending changes
      updateNoteSyncStatus(noteId, 'pending');
      
      // Persist state locally
      options.persistCrdtState?.(noteId, docManager.getState(noteId));
      
      // Push to server if connected
      const provider = getWebSocketSyncProvider();
      if (provider?.isConnected()) {
        provider.pushUpdate(noteId, update);
      }
    },
  });

  // Initialize WebSocket provider if URL provided
  let wsProvider = options.wsServerUrl ? getWebSocketSyncProvider({
    serverUrl: options.wsServerUrl,
    getAuthToken: options.getAuthToken,
    onConnectionChange: (state) => {
      connectionState = state;
      if (state === 'connected') {
        // Trigger sync for notes with pending updates
        triggerSync();
      }
    },
    onSyncComplete: (noteIds) => {
      for (const noteId of noteIds) {
        updateNoteSyncStatus(noteId, 'synced');
      }
      lastSyncTime = new Date().toISOString();
      syncError = null;
    },
    onSyncError: (error) => {
      syncError = error.message;
    },
  }) : null;

  /**
   * Update the sync status for a note
   */
  function updateNoteSyncStatus(noteId: string, state: NoteSyncStatus['state']): void {
    const current = noteSyncStatuses.get(noteId);
    noteSyncStatuses.set(noteId, {
      id: noteId,
      state,
      lastSyncedAt: state === 'synced' ? new Date().toISOString() : current?.lastSyncedAt,
    });
    // Trigger reactivity
    noteSyncStatuses = new Map(noteSyncStatuses);
  }

  /**
   * Initialize a note's CRDT document
   */
  async function initializeNote(noteId: string, initialContent?: string): Promise<void> {
    // Try to load existing state
    if (options.loadCrdtState) {
      const existingState = await options.loadCrdtState(noteId);
      if (existingState) {
        docManager.loadState(noteId, existingState);
        updateNoteSyncStatus(noteId, 'synced');
        return;
      }
    }

    // Initialize with content if provided
    if (initialContent) {
      docManager.initializeWithContent(noteId, initialContent);
    } else {
      // Just create an empty doc
      docManager.getDoc(noteId);
    }
    updateNoteSyncStatus(noteId, 'synced');
  }

  /**
   * Queue a metadata update for sync
   */
  function queueMetadataUpdate(update: NoteMetadataUpdate): void {
    // Replace existing update for same note or add new
    const index = pendingMetadataUpdates.findIndex(u => u.id === update.id);
    if (index >= 0) {
      pendingMetadataUpdates[index] = update;
    } else {
      pendingMetadataUpdates.push(update);
    }
    pendingMetadataUpdates = [...pendingMetadataUpdates];
  }

  /**
   * Trigger a sync with the server
   */
  async function triggerSync(): Promise<void> {
    // Get all notes that need syncing
    const noteIds = [
      ...docManager.getNotesWithPendingUpdates(),
      ...pendingMetadataUpdates.map(m => m.id),
    ];
    const uniqueNoteIds = [...new Set(noteIds)];

    if (uniqueNoteIds.length === 0 && pendingMetadataUpdates.length === 0) {
      return;
    }

    // Mark notes as syncing
    for (const noteId of uniqueNoteIds) {
      updateNoteSyncStatus(noteId, 'syncing');
    }

    // If WebSocket is connected, use it
    if (wsProvider?.isConnected()) {
      wsProvider.requestSync(uniqueNoteIds, pendingMetadataUpdates);
      pendingMetadataUpdates = [];
      return;
    }

    // Fall back to HTTP sync
    if (options.httpBaseUrl) {
      await httpSync(uniqueNoteIds);
    }
  }

  /**
   * HTTP-based sync fallback
   */
  async function httpSync(noteIds: string[]): Promise<void> {
    if (!options.httpBaseUrl) return;

    try {
      const request = docManager.prepareSyncRequest(noteIds, pendingMetadataUpdates);
      
      const token = options.getAuthToken();
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };
      if (token) {
        headers['Authorization'] = `Bearer ${token}`;
      }

      const response = await fetch(`${options.httpBaseUrl}/api/sync/crdt`, {
        method: 'POST',
        headers,
        body: JSON.stringify(request),
      });

      if (!response.ok) {
        throw new Error(`Sync failed: ${response.status}`);
      }

      const syncResponse: CrdtSyncResponse = await response.json();
      
      // Apply response
      docManager.applySyncResponse(syncResponse);
      
      // Update statuses
      for (const noteId of noteIds) {
        updateNoteSyncStatus(noteId, 'synced');
        // Persist updated state
        options.persistCrdtState?.(noteId, docManager.getState(noteId));
      }

      pendingMetadataUpdates = [];
      lastSyncTime = syncResponse.server_time;
      syncError = null;
    } catch (error) {
      syncError = (error as Error).message;
      // Revert to pending status
      for (const noteId of noteIds) {
        updateNoteSyncStatus(noteId, 'pending');
      }
    }
  }

  /**
   * Connect to the sync server
   */
  function connect(): void {
    wsProvider?.connect();
  }

  /**
   * Disconnect from the sync server
   */
  function disconnect(): void {
    wsProvider?.disconnect();
  }

  /**
   * Subscribe to real-time updates for a note
   */
  function subscribeToNote(noteId: string): void {
    wsProvider?.subscribeToNote(noteId);
  }

  /**
   * Unsubscribe from a note
   */
  function unsubscribeFromNote(noteId: string): void {
    wsProvider?.unsubscribeFromNote(noteId);
  }

  /**
   * Clean up a note's CRDT document
   */
  function cleanupNote(noteId: string): void {
    docManager.destroyDoc(noteId);
    noteSyncStatuses.delete(noteId);
    noteSyncStatuses = new Map(noteSyncStatuses);
  }

  /**
   * Get the Yjs XmlFragment for TipTap binding
   */
  function getYjsFragment(noteId: string) {
    return docManager.getXmlFragment(noteId);
  }

  /**
   * Get the Yjs Doc for a note
   */
  function getYjsDoc(noteId: string) {
    return docManager.getDoc(noteId);
  }

  /**
   * Destroy the sync store
   */
  function destroy(): void {
    wsProvider?.destroy();
    docManager.destroy();
  }

  return {
    // State
    get connectionState() { return connectionState; },
    get noteSyncStatuses() { return noteSyncStatuses; },
    get lastSyncTime() { return lastSyncTime; },
    get syncError() { return syncError; },

    // Document management
    initializeNote,
    cleanupNote,
    getYjsFragment,
    getYjsDoc,

    // Sync operations
    queueMetadataUpdate,
    triggerSync,
    connect,
    disconnect,
    subscribeToNote,
    unsubscribeFromNote,

    // Cleanup
    destroy,
  };
}

// Export type for the store
export type SyncStore = ReturnType<typeof createSyncStore>;
