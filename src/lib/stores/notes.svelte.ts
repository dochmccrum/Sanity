import type { Note, NoteInput, NoteSummary } from '$lib/api/notes';
import * as notesApi from '$lib/api/notes';

export function createNotesStore() {
  let notes = $state<NoteSummary[]>([]);
  let selectedNote = $state<Note | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function loadNotes(folderId?: string, uncategorisedOnly?: boolean) {
    loading = true;
    error = null;
    try {
      if (uncategorisedOnly) {
        // Load all notes and filter for those without a folder
        const allNotes = await notesApi.getAllNotes();
        notes = allNotes.filter(note => !note.folder_id);
      } else if (folderId) {
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

  async function moveNote(noteId: string, folderId: string | null) {
      try {
        await notesApi.moveNote(noteId, folderId);
        
        const noteIndex = notes.findIndex(n => n.id === noteId);
        if (noteIndex !== -1) {
            notes[noteIndex].folder_id = folderId;
            // update timestamp locally?
        }
        if (selectedNote?.id === noteId) {
             selectedNote.folder_id = folderId;
        }
      } catch (err) {
          console.error("Failed to move note", err);
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
        return;
    }

    // Otherwise, fetch full details
    try {
        const fullNote = await notesApi.getNote(note.id);
        if (fullNote) {
            selectedNote = fullNote;
        }
    } catch(e) {
        console.error("Failed to fetch full note", e);
    }
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
    moveNote,
    selectNote,
    clearError,
  };
}
