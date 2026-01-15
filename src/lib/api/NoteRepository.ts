import type { Note, NoteInput, SyncPayload, SyncResult } from '../types/note';

export interface NoteRepository {
  listNotes(folderId?: string | null): Promise<Note[]>;
  getNote(id: string): Promise<Note | null>;
  saveNote(note: NoteInput): Promise<Note>;
  deleteNote(id: string): Promise<boolean>;
  moveNote(id: string, folderId: string | null): Promise<Note>;
  sync(payload: SyncPayload): Promise<SyncResult>;
}
