import type { Folder, FolderInput } from '$lib/api/folders';
import * as foldersApi from '$lib/api/folders';

export function createFoldersStore() {
  let folders = $state<Folder[]>([]);
  let selectedFolder = $state<Folder | null | 'uncategorised'>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function loadFolders() {
    loading = true;
    error = null;
    try {
      folders = await foldersApi.getAllFolders();
      console.log('[FoldersStore] Loaded folders:', folders.length, folders.map(f => f.name));
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load folders';
      console.error('Error loading folders:', err);
    } finally {
      loading = false;
    }
  }

  async function createFolder(parentId: string | null = null): Promise<Folder | null> {
    error = null;
    try {
      const newFolder = foldersApi.createEmptyFolder(parentId);
      const savedFolder = await foldersApi.saveFolder(newFolder);
      folders = [...folders, savedFolder];
      return savedFolder;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to create folder';
      console.error('Error creating folder:', err);
      return null;
    }
  }

  async function updateFolder(folder: FolderInput): Promise<Folder | null> {
    error = null;
    try {
      const updatedFolder = await foldersApi.saveFolder(folder);
      const index = folders.findIndex((f) => f.id === updatedFolder.id);
      if (index !== -1) {
        folders[index] = updatedFolder;
      } else {
        folders = [...folders, updatedFolder];
      }
      return updatedFolder;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to update folder';
      console.error('Error updating folder:', err);
      return null;
    }
  }

  async function deleteFolder(id: string): Promise<boolean> {
    error = null;
    try {
      await foldersApi.deleteFolder(id);
      folders = folders.filter((f) => f.id !== id);
      if (selectedFolder && selectedFolder !== 'uncategorised' && selectedFolder.id === id) {
        selectedFolder = null;
      }
      return true;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to delete folder';
      console.error('Error deleting folder:', err);
      return false;
    }
  }

  function selectFolder(folder: Folder | null | 'uncategorised') {
    selectedFolder = folder;
  }

  function clearError() {
    error = null;
  }

  return {
    get folders() { return folders; },
    get selectedFolder() { return selectedFolder; },
    set selectedFolder(value: Folder | null | 'uncategorised') { selectedFolder = value; },
    get loading() { return loading; },
    get error() { return error; },
    loadFolders,
    createFolder,
    updateFolder,
    deleteFolder,
    selectFolder,
    clearError,
  };
}
