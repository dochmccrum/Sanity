import { TauriAdapter } from './adapters/TauriAdapter';
import { WebAdapter } from './adapters/WebAdapter';
import type { NoteRepository } from './NoteRepository';
import { env } from '$env/dynamic/public';

let instance: NoteRepository | null = null;

export function getNoteRepository(): NoteRepository {
  if (instance) return instance;

  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
  const baseUrl = env.PUBLIC_API_BASE_URL ?? '';
  instance = isTauri ? new TauriAdapter() : new WebAdapter(baseUrl);
  return instance;
}
