import type { Note, NoteInput } from '$lib/api/notes';
import * as notesApi from '$lib/api/notes';

export function createNotesStore() {
  let notes = $state<Note[]>([]);
  let selectedNote = $state<Note | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function loadNotes(folderId?: string) {
    loading = true;
    error = null;
    try {
      if (folderId) {
        notes = await notesApi.getNotesByFolder(folderId);
      } else {
        notes = await notesApi.getAllNotes();
      }
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
      const newNote = await notesApi.saveNote({
        title: 'Untitled Note',
        content: '',
        folder_id: folderId,
        is_canvas: false,
      });
      notes = [newNote, ...notes];
      selectedNote = newNote;
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
      await notesApi.saveNote({
        id: note.id,
        title: note.title,
        content: note.content,
        folder_id: note.folder_id,
        is_canvas: note.is_canvas,
      });
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

  async function deleteNote(id: string) {
    error = null;
    try {
      await notesApi.deleteNote(id);
      notes = notes.filter((n) => n.id !== id);
      if (selectedNote?.id === id) {
        selectedNote = null;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to delete note';
      console.error('Error deleting note:', err);
    }
  }

  function selectNote(note: Note | null) {
    selectedNote = note;
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
    deleteNote,
    selectNote,
    clearError,
  };
}
