/**
 * TypeScript bindings for the Tauri backend commands
 * Auto-generated types to match the Rust structures
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface Note {
  id: string;
  title: string;
  content: string;
  folder_id: string | null;
  updated_at: string;
  is_canvas: boolean;
}

export interface NoteInput {
  id?: string;
  title: string;
  content: string;
  folder_id?: string | null;
  is_canvas: boolean;
}

export interface AssetResult {
  id: string;
  uri: string;
  path: string;
}

export interface CommandError {
  message: string;
}

// ============================================================================
// Note API
// ============================================================================

/**
 * Get all notes from the database
 */
export async function getAllNotes(): Promise<Note[]> {
  return invoke<Note[]>('get_all_notes');
}

/**
 * Get a single note by ID
 */
export async function getNote(id: string): Promise<Note | null> {
  return invoke<Note | null>('get_note', { id });
}

/**
 * Get notes by folder ID (pass undefined/null for root-level notes)
 */
export async function getNotesByFolder(folderId?: string | null): Promise<Note[]> {
  return invoke<Note[]>('get_notes_by_folder', { folderId });
}

/**
 * Save a note (create new or update existing)
 * - Omit `id` to create a new note with auto-generated UUID
 * - Include `id` to update an existing note
 */
export async function saveNote(note: NoteInput): Promise<Note> {
  return invoke<Note>('save_note', { note });
}

/**
 * Delete a note by ID
 * Returns true if a note was deleted, false if not found
 */
export async function deleteNote(id: string): Promise<boolean> {
  return invoke<boolean>('delete_note', { id });
}

// ============================================================================
// Asset API
// ============================================================================

/**
 * Save an image from base64 data
 * @param base64Data - Base64 encoded image data (with or without data URL prefix)
 * @param fileExtension - File extension (e.g., 'png', 'jpg', 'webp')
 * @returns Asset info including the local URI for display
 */
export async function saveImageAsset(
  base64Data: string,
  fileExtension: string
): Promise<AssetResult> {
  return invoke<AssetResult>('save_image_asset', { base64Data, fileExtension });
}

/**
 * Save raw image bytes as an asset
 * @param data - Raw byte array of the image
 * @param fileExtension - File extension (e.g., 'png', 'jpg', 'webp')
 */
export async function saveImageBytes(
  data: number[],
  fileExtension: string
): Promise<AssetResult> {
  return invoke<AssetResult>('save_image_bytes', { data, fileExtension });
}

/**
 * Delete an asset by its ID
 */
export async function deleteAsset(assetId: string): Promise<boolean> {
  return invoke<boolean>('delete_asset', { assetId });
}

/**
 * List all saved assets
 */
export async function listAssets(): Promise<AssetResult[]> {
  return invoke<AssetResult[]>('list_assets');
}

/**
 * Get the path to the assets directory
 */
export async function getAssetsPath(): Promise<string> {
  return invoke<string>('get_assets_path');
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Helper to convert a File object to base64 and save as asset
 */
export async function saveFileAsAsset(file: File): Promise<AssetResult> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = async () => {
      try {
        const base64 = reader.result as string;
        const extension = file.name.split('.').pop() || 'png';
        const result = await saveImageAsset(base64, extension);
        resolve(result);
      } catch (error) {
        reject(error);
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/**
 * Create a new empty note
 */
export function createEmptyNote(folderId?: string | null): NoteInput {
  return {
    title: '',
    content: '',
    folder_id: folderId ?? null,
    is_canvas: false,
  };
}

/**
 * Create a new canvas note
 */
export function createCanvasNote(folderId?: string | null): NoteInput {
  return {
    title: '',
    content: '{}', // Empty canvas JSON
    folder_id: folderId ?? null,
    is_canvas: true,
  };
}
