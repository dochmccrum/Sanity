import type { NoteRepository, CrdtState } from '../NoteRepository';
import type { Note, NoteInput, SyncPayload, SyncResult, CrdtSyncRequest, CrdtSyncResponse } from '../../types/note';
import { base64ToUint8Array, uint8ArrayToBase64 } from '../../sync/YjsDocManager';

function authHeader() {
  const token = typeof window !== 'undefined' ? localStorage.getItem('jwt') : null;
  const headers: Record<string, string> = {};
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

async function fetchJson<T>(input: RequestInfo | URL, init?: RequestInit): Promise<T> {
  const res = await fetch(input, init);
  if (!res.ok) {
    throw new Error(`Request failed: ${res.status}`);
  }
  return (await res.json()) as T;
}

interface ServerCrdtState {
  note_id: string;
  ydoc_state: string;  // base64
  state_vector: string; // base64
  updated_at: string;
}

export class WebAdapter implements NoteRepository {
  constructor(private readonly baseUrl = '') {}

  async listNotes(folderId?: string | null): Promise<Note[]> {
    const query = typeof folderId === 'undefined' ? '' : `?folder_id=${encodeURIComponent(folderId ?? 'null')}`;
    return fetchJson<Note[]>(`${this.baseUrl}/api/notes${query}`, {
      headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
    });
  }

  async getNote(id: string): Promise<Note | null> {
    return fetchJson<Note>(`${this.baseUrl}/api/notes/${id}`, {
      headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
    });
  }

  async saveNote(note: NoteInput): Promise<Note> {
    return fetchJson<Note>(`${this.baseUrl}/api/notes`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
      body: JSON.stringify(note),
    });
  }

  async deleteNote(id: string): Promise<boolean> {
    await fetchJson(`${this.baseUrl}/api/notes/${id}`, {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
    });
    return true;
  }

  async moveNote(id: string, folderId: string | null): Promise<Note> {
    const existing = await this.getNote(id);
    const payload: NoteInput = existing
      ? { ...existing, folder_id: folderId }
      : { id, title: '', content: '', folder_id: folderId, is_deleted: false, is_canvas: false };
    return this.saveNote(payload);
  }

  async sync(payload: SyncPayload): Promise<SyncResult> {
    return fetchJson<SyncResult>(`${this.baseUrl}/api/sync`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
      body: JSON.stringify(payload),
    });
  }

  async syncCrdt(request: CrdtSyncRequest): Promise<CrdtSyncResponse> {
    return fetchJson<CrdtSyncResponse>(`${this.baseUrl}/api/sync/crdt`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
      body: JSON.stringify(request),
    });
  }

  /**
   * Get CRDT state for a note from the server
   * This allows the web app to load existing CRDT state created by other clients
   */
  async getCrdtState(noteId: string): Promise<CrdtState | null> {
    try {
      const response = await fetchJson<ServerCrdtState | null>(
        `${this.baseUrl}/api/crdt/${noteId}`,
        {
          headers: { 'Content-Type': 'application/json', ...authHeader() } as Record<string, string>,
        }
      );
      
      if (!response) return null;
      
      return {
        note_id: response.note_id,
        ydoc_state: base64ToUint8Array(response.ydoc_state),
        state_vector: base64ToUint8Array(response.state_vector),
        updated_at: response.updated_at,
      };
    } catch (error) {
      // If the endpoint doesn't exist or fails, return null
      console.warn('Failed to fetch CRDT state:', error);
      return null;
    }
  }

  /**
   * Get WebSocket URL for real-time sync
   */
  getWebSocketUrl(): string {
    const protocol = this.baseUrl.startsWith('https') ? 'wss' : 'ws';
    const host = this.baseUrl.replace(/^https?:\/\//, '') || window.location.host;
    const token = typeof window !== 'undefined' ? localStorage.getItem('jwt') : null;
    const tokenParam = token ? `?token=${encodeURIComponent(token)}` : '';
    return `${protocol}://${host}/api/ws${tokenParam}`;
  }
}
