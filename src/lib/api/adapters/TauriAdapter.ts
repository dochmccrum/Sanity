import { getAllNotes, getNote as tauriGetNote, getNotesByFolder, saveNote as tauriSaveNote, deleteNote as tauriDeleteNote, moveNote as tauriMoveNote } from '../notes';
import type { NoteRepository } from '../NoteRepository';
import type { Note, NoteInput, SyncPayload, SyncResult } from '../../types/note';

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
  async listNotes(folderId?: string | null): Promise<Note[]> {
    const notes = folderId ? await getNotesByFolder(folderId) : await getAllNotes();
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
      is_deleted: note.is_deleted ?? false,
      is_canvas: note.is_canvas ?? false,
    });
    return mapToShared(saved);
  }

  async deleteNote(id: string): Promise<boolean> {
    return tauriDeleteNote(id);
  }

  async moveNote(id: string, folderId: string | null): Promise<Note> {
    await tauriMoveNote(id, folderId);
    const note = await this.getNote(id);
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
    return {
      pulled: [],
      last_sync: new Date().toISOString(),
    };
  }
}
