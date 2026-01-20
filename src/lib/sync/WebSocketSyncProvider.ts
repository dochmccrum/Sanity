/**
 * WebSocket Sync Provider for Yjs
 * 
 * Handles real-time synchronization with the server via WebSockets.
 * Implements the y-protocols for efficient CRDT sync.
 */

import type { ConnectionState, WsMessage, CrdtSyncResponse, NoteMetadataUpdate } from '$lib/types/note';
import { getYjsDocManager, uint8ArrayToBase64, base64ToUint8Array } from './YjsDocManager';

export interface SyncProviderOptions {
  /** WebSocket URL for the sync server */
  serverUrl: string;
  /** JWT token for authentication */
  getAuthToken: () => string | null;
  /** Reconnect delay in milliseconds */
  reconnectDelay?: number;
  /** Max reconnect attempts */
  maxReconnectAttempts?: number;
  /** Callback when connection state changes */
  onConnectionChange?: (state: ConnectionState) => void;
  /** Callback when sync completes */
  onSyncComplete?: (noteIds: string[]) => void;
  /** Callback when note metadata is updated */
  onMetadataUpdate?: (metadata: NoteMetadataUpdate) => void;
  /** Callback on sync error */
  onSyncError?: (error: Error) => void;
}

/**
 * WebSocket-based sync provider for real-time CRDT synchronization
 */
export class WebSocketSyncProvider {
  private ws: WebSocket | null = null;
  private options: SyncProviderOptions;
  private connectionState: ConnectionState = 'disconnected';
  private reconnectAttempts = 0;
  private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
  private subscribedNotes: Set<string> = new Set();
  private pendingMessages: WsMessage[] = [];
  private syncInProgress = false;

  constructor(options: SyncProviderOptions) {
    this.options = {
      reconnectDelay: 1000,
      maxReconnectAttempts: 10,
      ...options,
    };
  }

  /**
   * Connect to the sync server
   */
  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      return;
    }

    this.setConnectionState('connecting');
    
    const token = this.options.getAuthToken();
    const url = token 
      ? `${this.options.serverUrl}?token=${encodeURIComponent(token)}`
      : this.options.serverUrl;

    try {
      this.ws = new WebSocket(url);
      this.setupWebSocketHandlers();
    } catch (error) {
      this.handleConnectionError(error as Error);
    }
  }

  /**
   * Disconnect from the sync server
   */
  disconnect(): void {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }
    
    if (this.ws) {
      this.ws.close(1000, 'Client disconnect');
      this.ws = null;
    }
    
    this.setConnectionState('disconnected');
    this.reconnectAttempts = 0;
  }

  /**
   * Subscribe to real-time updates for a note
   */
  subscribeToNote(noteId: string): void {
    this.subscribedNotes.add(noteId);
    
    if (this.isConnected()) {
      this.sendMessage({
        type: 'subscribe',
        note_id: noteId,
        payload: '',
      });
    }
  }

  /**
   * Unsubscribe from real-time updates for a note
   */
  unsubscribeFromNote(noteId: string): void {
    this.subscribedNotes.delete(noteId);
    
    if (this.isConnected()) {
      this.sendMessage({
        type: 'unsubscribe',
        note_id: noteId,
        payload: '',
      });
    }
  }

  /**
   * Push a local update to the server
   */
  pushUpdate(noteId: string, update: Uint8Array): void {
    const message: WsMessage = {
      type: 'update',
      note_id: noteId,
      payload: uint8ArrayToBase64(update),
    };

    if (this.isConnected()) {
      this.sendMessage(message);
    } else {
      // Queue for later
      this.pendingMessages.push(message);
    }
  }

  /**
   * Push note metadata to the server
   */
  pushMetadata(metadata: NoteMetadataUpdate): void {
    const message: WsMessage = {
      type: 'note_metadata',
      payload: JSON.stringify(metadata),
    };

    if (this.isConnected()) {
      this.sendMessage(message);
    } else {
      this.pendingMessages.push(message);
    }
  }

  /**
   * Request a full sync for specific notes
   */
  requestSync(noteIds: string[], metadata: NoteMetadataUpdate[]): void {
    if (!this.isConnected() || this.syncInProgress) {
      return;
    }

    this.syncInProgress = true;
    this.setConnectionState('syncing');

    const docManager = getYjsDocManager();
    const request = docManager.prepareSyncRequest(noteIds, metadata);

    this.sendMessage({
      type: 'sync_request',
      payload: JSON.stringify(request),
    });
  }

  /**
   * Check if connected to server
   */
  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /**
   * Get current connection state
   */
  getConnectionState(): ConnectionState {
    return this.connectionState;
  }

  /**
   * Destroy the provider
   */
  destroy(): void {
    this.disconnect();
    this.subscribedNotes.clear();
    this.pendingMessages = [];
  }

  // Private methods

  private setupWebSocketHandlers(): void {
    if (!this.ws) return;

    this.ws.onopen = () => {
      this.setConnectionState('connected');
      this.reconnectAttempts = 0;
      
      // Re-subscribe to all notes
      for (const noteId of this.subscribedNotes) {
        this.sendMessage({
          type: 'subscribe',
          note_id: noteId,
          payload: '',
        });
      }

      // Send any pending messages
      this.flushPendingMessages();
    };

    this.ws.onclose = (event) => {
      const wasConnected = this.connectionState === 'connected';
      this.setConnectionState('disconnected');
      this.ws = null;

      // Attempt reconnect if it wasn't a clean close
      if (event.code !== 1000 && wasConnected) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = (event) => {
      console.error('WebSocket error:', event);
      this.options.onSyncError?.(new Error('WebSocket connection error'));
    };

    this.ws.onmessage = (event) => {
      this.handleMessage(event.data);
    };
  }

  private handleMessage(data: string): void {
    try {
      const message: WsMessage = JSON.parse(data);
      
      switch (message.type) {
        case 'sync_response':
          this.handleSyncResponse(message);
          break;
        case 'update':
          this.handleRemoteUpdate(message);
          break;
        case 'note_metadata':
          this.handleMetadataUpdate(message);
          break;
        case 'awareness':
          // Handle awareness (cursors, presence) - future enhancement
          break;
        default:
          console.warn('Unknown message type:', message.type);
      }
    } catch (error) {
      console.error('Failed to parse WebSocket message:', error);
    }
  }

  private handleMetadataUpdate(message: WsMessage): void {
    try {
      const metadata: NoteMetadataUpdate = JSON.parse(message.payload);
      this.options.onMetadataUpdate?.(metadata);
    } catch (error) {
      console.error('Failed to parse metadata update:', error);
    }
  }

  private handleSyncResponse(message: WsMessage): void {
    this.syncInProgress = false;
    this.setConnectionState('connected');

    try {
      const response: CrdtSyncResponse = JSON.parse(message.payload);
      const docManager = getYjsDocManager();
      
      // Apply all updates from the server
      docManager.applySyncResponse(response);
      
      // Notify metadata updates
      if (response.metadata && response.metadata.length > 0) {
        for (const meta of response.metadata) {
          this.options.onMetadataUpdate?.(meta);
        }
      }
      
      // Notify completion
      const syncedNoteIds = Object.keys(response.updates);
      this.options.onSyncComplete?.(syncedNoteIds);
    } catch (error) {
      this.options.onSyncError?.(error as Error);
    }
  }

  private handleRemoteUpdate(message: WsMessage): void {
    if (!message.note_id) return;

    const docManager = getYjsDocManager();
    docManager.applyUpdateFromBase64(message.note_id, message.payload, 'remote');
  }

  private sendMessage(message: WsMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  private flushPendingMessages(): void {
    while (this.pendingMessages.length > 0) {
      const message = this.pendingMessages.shift();
      if (message) {
        this.sendMessage(message);
      }
    }
  }

  private setConnectionState(state: ConnectionState): void {
    if (this.connectionState !== state) {
      this.connectionState = state;
      this.options.onConnectionChange?.(state);
    }
  }

  private handleConnectionError(error: Error): void {
    this.setConnectionState('disconnected');
    this.options.onSyncError?.(error);
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= (this.options.maxReconnectAttempts || 10)) {
      console.error('Max reconnect attempts reached');
      return;
    }

    if (this.reconnectTimeout) {
      return;
    }

    const delay = (this.options.reconnectDelay || 1000) * Math.pow(2, this.reconnectAttempts);
    this.reconnectAttempts++;

    this.reconnectTimeout = setTimeout(() => {
      this.reconnectTimeout = null;
      this.connect();
    }, delay);
  }
}

// Singleton instance
let globalProvider: WebSocketSyncProvider | null = null;

export function getWebSocketSyncProvider(options?: SyncProviderOptions): WebSocketSyncProvider | null {
  if (options && !globalProvider) {
    globalProvider = new WebSocketSyncProvider(options);
  }
  return globalProvider;
}

export function destroyWebSocketSyncProvider(): void {
  if (globalProvider) {
    globalProvider.destroy();
    globalProvider = null;
  }
}
