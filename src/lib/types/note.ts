export interface Note {
  id: string;
  title: string;
  content: string;
  folder_id: string | null;
  updated_at: string;
  is_deleted: boolean;
  is_canvas: boolean;
}

export type NoteSummary = Note;

export type NoteInput = Omit<Note, 'id' | 'updated_at'> & {
  id?: string;
  updated_at?: string;
};

export interface SyncPayload {
  since?: string;
  notes: Note[];
}

export interface SyncResult {
  pulled: Note[];
  last_sync: string;
}
