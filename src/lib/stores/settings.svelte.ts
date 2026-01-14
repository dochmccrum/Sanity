export function createSettingsStore() {
  let autoFocusTitleOnNewNote = $state(true);
  let confirmBeforeDelete = $state(true);
  let enableAutoComplete = $state(true);
  let showAllNotesFolder = $state(true);
  let showUncategorisedFolder = $state(true);

  return {
    get autoFocusTitleOnNewNote() {
      return autoFocusTitleOnNewNote;
    },
    set autoFocusTitleOnNewNote(value: boolean) {
      autoFocusTitleOnNewNote = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('jfnotes_auto_focus', JSON.stringify(value));
      }
    },
    get confirmBeforeDelete() {
      return confirmBeforeDelete;
    },
    set confirmBeforeDelete(value: boolean) {
      confirmBeforeDelete = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('jfnotes_confirm_delete', JSON.stringify(value));
      }
    },
    get enableAutoComplete() {
      return enableAutoComplete;
    },
    set enableAutoComplete(value: boolean) {
      enableAutoComplete = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('jfnotes_auto_complete', JSON.stringify(value));
      }
    },
    get showAllNotesFolder() {
      return showAllNotesFolder;
    },
    set showAllNotesFolder(value: boolean) {
      showAllNotesFolder = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('jfnotes_show_all_notes', JSON.stringify(value));
      }
    },
    get showUncategorisedFolder() {
      return showUncategorisedFolder;
    },
    set showUncategorisedFolder(value: boolean) {
      showUncategorisedFolder = value;
      if (typeof window !== 'undefined') {
        localStorage.setItem('jfnotes_show_uncategorised', JSON.stringify(value));
      }
    },
    loadSettings() {
      if (typeof window !== 'undefined') {
        const savedAutoFocus = localStorage.getItem('jfnotes_auto_focus');
        if (savedAutoFocus !== null) {
          autoFocusTitleOnNewNote = JSON.parse(savedAutoFocus);
        }
        const savedConfirmDelete = localStorage.getItem('jfnotes_confirm_delete');
        if (savedConfirmDelete !== null) {
          confirmBeforeDelete = JSON.parse(savedConfirmDelete);
        }
        const savedAutoComplete = localStorage.getItem('jfnotes_auto_complete');
        if (savedAutoComplete !== null) {
          enableAutoComplete = JSON.parse(savedAutoComplete);
        }
        const savedShowAllNotes = localStorage.getItem('jfnotes_show_all_notes');
        if (savedShowAllNotes !== null) {
          showAllNotesFolder = JSON.parse(savedShowAllNotes);
        }
        const savedShowUncategorised = localStorage.getItem('jfnotes_show_uncategorised');
        if (savedShowUncategorised !== null) {
          showUncategorisedFolder = JSON.parse(savedShowUncategorised);
        }
      }
    }
  };
}
