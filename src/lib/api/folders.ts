import { env } from '$env/dynamic/public';

const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;

function getBaseUrl(): string {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('beck_sync_server_url');
    if (saved) return saved.replace(/\/+$/, '');
  }
  return env.PUBLIC_API_BASE_URL || '';
}

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
  updated_at: string;
  is_deleted: boolean;
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
  return fetchJson<Folder[]>(`${getBaseUrl()}/api/folders`);
}

/**
 * Get a single folder by ID
 */
export async function getFolder(id: string): Promise<Folder | null> {
  if (isTauri) {
    return await tauriInvoke<Folder | null>('get_folder', { id });
  }
  return fetchJson<Folder>(`${getBaseUrl()}/api/folders/${id}`);
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
  return fetchJson<Folder[]>(`${getBaseUrl()}/api/folders?parent_id=${encodeURIComponent(query ?? '')}`);
}

/**
 * Save a folder (create or update)
 */
export async function saveFolder(folder: FolderInput): Promise<Folder> {
  if (isTauri) {
    return await tauriInvoke<Folder>('save_folder', { folder });
  }
  return fetchJson<Folder>(`${getBaseUrl()}/api/folders`, {
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
  await fetchJson(`${getBaseUrl()}/api/folders/${id}`, { method: 'DELETE' });
}

// ============================================================================
// Sync helpers (Tauri-only)
// ============================================================================

/**
 * Get locally updated folders since a timestamp (RFC3339). Includes deleted folders.
 */
export async function getFoldersUpdatedSince(since?: string | null): Promise<Folder[]> {
  if (!isTauri) return [];
  return tauriInvoke<Folder[]>('get_folders_updated_since', { since });
}

/**
 * Apply folders pulled from the remote server.
 */
export async function applySyncFolders(folders: Folder[]): Promise<void> {
  if (!isTauri) return;
  return tauriInvoke<void>('apply_sync_folders', { folders });
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
