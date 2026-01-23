export function createSettingsStore() {
  let autoFocusTitleOnNewNote = $state(true);
  let autoSelectFolderNameOnEdit = $state(true);
  let confirmNoteDelete = $state(true);
  let confirmFolderDelete = $state(true);
  let enableAutoComplete = $state(true);
  let showAllNotesFolder = $state(true);
  let showUncategorisedFolder = $state(true);
  let showNotePreviews = $state(true);
  let singleSidebarMode = $state(false);
  let syncServerUrl = $state('');
  let syncUsername = $state('');
  let lastSync = $state<string | null>(null);
  
  let shortcuts = $state({
    toggleSidebar: 'Alt+b',
    search: 'Control+k',
    newNote: 'Alt+n',
    newFolder: 'Alt+Shift+n',
    exportPdf: 'Alt+e'
  });

  return {
    get autoFocusTitleOnNewNote() {
      return autoFocusTitleOnNewNote;
    },
    set autoFocusTitleOnNewNote(value: boolean) {
      autoFocusTitleOnNewNote = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_auto_focus', JSON.stringify(value));
      }
    },
    get autoSelectFolderNameOnEdit() {
      return autoSelectFolderNameOnEdit;
    },
    set autoSelectFolderNameOnEdit(value: boolean) {
      autoSelectFolderNameOnEdit = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_auto_select_folder', JSON.stringify(value));
      }
    },
    get confirmNoteDelete() {
      return confirmNoteDelete;
    },
    set confirmNoteDelete(value: boolean) {
      confirmNoteDelete = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_confirm_note_delete', JSON.stringify(value));
      }
    },
    get confirmFolderDelete() {
      return confirmFolderDelete;
    },
    set confirmFolderDelete(value: boolean) {
      confirmFolderDelete = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_confirm_folder_delete', JSON.stringify(value));
      }
    },
    get enableAutoComplete() {
      return enableAutoComplete;
    },
    set enableAutoComplete(value: boolean) {
      enableAutoComplete = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_auto_complete', JSON.stringify(value));
      }
    },
    get showAllNotesFolder() {
      return showAllNotesFolder;
    },
    set showAllNotesFolder(value: boolean) {
      showAllNotesFolder = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_show_all_notes', JSON.stringify(value));
      }
    },
    get showUncategorisedFolder() {
      return showUncategorisedFolder;
    },
    set showUncategorisedFolder(value: boolean) {
      showUncategorisedFolder = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_show_uncategorised', JSON.stringify(value));
      }
    },
    get showNotePreviews() {
      return showNotePreviews;
    },
    set showNotePreviews(value: boolean) {
      showNotePreviews = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_show_previews', JSON.stringify(value));
      }
    },

    get singleSidebarMode() {
      return singleSidebarMode;
    },
    set singleSidebarMode(value: boolean) {
      singleSidebarMode = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_single_sidebar', JSON.stringify(value));
      }
    },

    get shortcuts() {
      return shortcuts;
    },
    updateShortcut(key: keyof typeof shortcuts, value: string) {
      shortcuts[key] = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_shortcuts', JSON.stringify(shortcuts));
      }
    },

    get syncServerUrl() {
      return syncServerUrl;
    },
    set syncServerUrl(value: string) {
      syncServerUrl = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_sync_server_url', value);
      }
    },

    get syncUsername() {
      return syncUsername;
    },
    set syncUsername(value: string) {
      syncUsername = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('beck_sync_username', value);
      }
    },

    get lastSync() {
      return lastSync;
    },

    refreshLastSync() {
      if (typeof window !== 'undefined') {
        lastSync = localStorage.getItem('beck_last_sync');
      }
    },

    async loginSync(password = ''): Promise<string> {
      if (typeof window === 'undefined') {
        throw new Error('loginSync can only run in the browser');
      }

      const base = (syncServerUrl ?? '').trim().replace(/\/+$/, '');
      const url = `${base}/api/auth`;

      const res = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: syncUsername, password }),
      });
      if (!res.ok) {
        throw new Error(`Login failed: ${res.status}`);
      }
      const json = (await res.json()) as { token: string };
      localStorage.setItem('jwt', json.token);
      return json.token;
    },

    logoutSync() {
      if (typeof window !== 'undefined') {
        localStorage.removeItem('jwt');
      }
    },

    loadSettings() {
      if (typeof window !== 'undefined') {
        const savedAutoFocus = localStorage.getItem('beck_auto_focus');
        if (savedAutoFocus !== null) {
          autoFocusTitleOnNewNote = JSON.parse(savedAutoFocus);
        }
        const savedAutoSelectFolder = localStorage.getItem('beck_auto_select_folder');
        if (savedAutoSelectFolder !== null) {
          autoSelectFolderNameOnEdit = JSON.parse(savedAutoSelectFolder);
        }
        
        // Handle migration from old combined setting or load new ones
        const savedNoteDelete = localStorage.getItem('beck_confirm_note_delete');
        const savedFolderDelete = localStorage.getItem('beck_confirm_folder_delete');
        const savedOldDelete = localStorage.getItem('beck_confirm_delete');

        if (savedNoteDelete !== null) {
          confirmNoteDelete = JSON.parse(savedNoteDelete);
        } else if (savedOldDelete !== null) {
          confirmNoteDelete = JSON.parse(savedOldDelete);
        }

        if (savedFolderDelete !== null) {
          confirmFolderDelete = JSON.parse(savedFolderDelete);
        } else if (savedOldDelete !== null) {
          confirmFolderDelete = JSON.parse(savedOldDelete);
        }

        const savedAutoComplete = localStorage.getItem('beck_auto_complete');
        if (savedAutoComplete !== null) {
          enableAutoComplete = JSON.parse(savedAutoComplete);
        }
        const savedShowAllNotes = localStorage.getItem('beck_show_all_notes');
        if (savedShowAllNotes !== null) {
          showAllNotesFolder = JSON.parse(savedShowAllNotes);
        }
        const savedShowUncategorised = localStorage.getItem('beck_show_uncategorised');
        if (savedShowUncategorised !== null) {
          showUncategorisedFolder = JSON.parse(savedShowUncategorised);
        }
        const savedShowPreviews = localStorage.getItem('beck_show_previews');
        if (savedShowPreviews !== null) {
          showNotePreviews = JSON.parse(savedShowPreviews);
        }
        const savedSingleSidebar = localStorage.getItem('beck_single_sidebar');
        if (savedSingleSidebar !== null) {
          singleSidebarMode = JSON.parse(savedSingleSidebar);
        }

        const savedShortcuts = localStorage.getItem('beck_shortcuts');
        if (savedShortcuts !== null) {
          const parsed = JSON.parse(savedShortcuts);
          Object.assign(shortcuts, parsed);
        }

        const savedSyncUrl = localStorage.getItem('beck_sync_server_url');
        if (savedSyncUrl !== null) {
          syncServerUrl = savedSyncUrl;
        }
        const savedSyncUsername = localStorage.getItem('beck_sync_username');
        if (savedSyncUsername !== null) {
          syncUsername = savedSyncUsername;
        }
        lastSync = localStorage.getItem('beck_last_sync');
      }
    }
  };
}
