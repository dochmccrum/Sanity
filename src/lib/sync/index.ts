/**
 * CRDT Sync Module
 * 
 * Exports all sync-related functionality for the Beck app.
 */

export { YjsDocManager, getYjsDocManager, destroyYjsDocManager, uint8ArrayToBase64, base64ToUint8Array } from './YjsDocManager';
export { WebSocketSyncProvider, getWebSocketSyncProvider, destroyWebSocketSyncProvider } from './WebSocketSyncProvider';
export { createSyncStore, type SyncStore } from './SyncStore.svelte';
