import { env } from '$env/dynamic/public';

const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
const baseUrl = env.PUBLIC_API_BASE_URL ?? '';

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

async function fetchJson<T>(input: RequestInfo | URL, init?: RequestInit): Promise<T> {
  const res = await fetch(input, init);
  if (!res.ok) {
    throw new Error(`Request failed: ${res.status}`);
  }
  return (await res.json()) as T;
}

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
  if (isTauri) {
    return await tauriInvoke<Folder[]>('get_all_folders');
  }
  return fetchJson<Folder[]>(`${baseUrl}/api/folders`);
}

/**
 * Get a single folder by ID
 */
export async function getFolder(id: string): Promise<Folder | null> {
  if (isTauri) {
    return await tauriInvoke<Folder | null>('get_folder', { id });
  }
  return fetchJson<Folder>(`${baseUrl}/api/folders/${id}`);
}

/**
 * Get child folders of a parent (or root folders if parent_id is null)
 */
export async function getFoldersByParent(parentId: string | null = null): Promise<Folder[]> {
  if (isTauri) {
    return await tauriInvoke<Folder[]>('get_folders_by_parent', { 
      parentId: parentId || undefined 
    });
  }
  const query = parentId === null ? 'null' : parentId;
  return fetchJson<Folder[]>(`${baseUrl}/api/folders?parent_id=${encodeURIComponent(query ?? '')}`);
}

/**
 * Save a folder (create or update)
 */
export async function saveFolder(folder: FolderInput): Promise<Folder> {
  if (isTauri) {
    return await tauriInvoke<Folder>('save_folder', { folder });
  }
  return fetchJson<Folder>(`${baseUrl}/api/folders`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(folder),
  });
}

/**
 * Delete a folder by ID
 */
export async function deleteFolder(id: string): Promise<void> {
  if (isTauri) {
    return await tauriInvoke<void>('delete_folder', { id });
  }
  await fetchJson(`${baseUrl}/api/folders/${id}`, { method: 'DELETE' });
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
