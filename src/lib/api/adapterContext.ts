import { TauriAdapter } from './adapters/TauriAdapter';
import { WebAdapter } from './adapters/WebAdapter';
import type { NoteRepository } from './NoteRepository';
import { env } from '$env/dynamic/public';

let instance: NoteRepository | null = null;

export function getNoteRepository(): NoteRepository {
  if (instance) return instance;

  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
  
  if (isTauri) {
    instance = new TauriAdapter();
  } else {
    // For web, use a dynamic base URL that respects settings
    const getBaseUrl = () => {
      if (typeof window !== 'undefined') {
        const saved = localStorage.getItem('beck_sync_server_url');
        if (saved) return saved.replace(/\/+$/, '');
      }
      return env.PUBLIC_API_BASE_URL || '';
    };
    instance = new WebAdapter(getBaseUrl);
  }
  
  return instance;
}
