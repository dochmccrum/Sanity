import type { NoteRepository } from '../NoteRepository';
import type { Note, NoteInput, SyncPayload, SyncResult } from '../../types/note';

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

export class WebAdapter implements NoteRepository {
  constructor(private readonly baseUrl = '') {}

  async listNotes(): Promise<Note[]> {
    return fetchJson<Note[]>(`${this.baseUrl}/api/notes`, {
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
}
