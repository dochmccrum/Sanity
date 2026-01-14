import { invoke } from '@tauri-apps/api/core';

export interface Folder {
  id: string;
  name: string;
  parent_id: string | null;
  created_at: string;
}

export interface FolderInput {
  id?: string;
  name: string;
  parent_id?: string | null;
}

/**
 * Get all folders from the database
 */
export async function getAllFolders(): Promise<Folder[]> {
  return await invoke<Folder[]>('get_all_folders');
}

/**
 * Get a single folder by ID
 */
export async function getFolder(id: string): Promise<Folder | null> {
  return await invoke<Folder | null>('get_folder', { id });
}

/**
 * Get child folders of a parent (or root folders if parent_id is null)
 */
export async function getFoldersByParent(parentId: string | null = null): Promise<Folder[]> {
  return await invoke<Folder[]>('get_folders_by_parent', { 
    parentId: parentId || undefined 
  });
}

/**
 * Save a folder (create or update)
 */
export async function saveFolder(folder: FolderInput): Promise<Folder> {
  return await invoke<Folder>('save_folder', { folder });
}

/**
 * Delete a folder by ID
 */
export async function deleteFolder(id: string): Promise<void> {
  return await invoke<void>('delete_folder', { id });
}

/**
 * Create an empty folder with a default name
 */
export function createEmptyFolder(parentId: string | null = null): FolderInput {
  return {
    name: 'New Folder',
    parent_id: parentId,
  };
}
