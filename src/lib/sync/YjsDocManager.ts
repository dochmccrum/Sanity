/**
 * Yjs Document Manager
 * 
 * Manages Yjs documents for CRDT-based conflict-free sync.
 * Each note has its own Y.Doc that tracks all changes.
 */

import * as Y from 'yjs';
import { browser } from '$app/environment';
import type { CrdtNoteState, CrdtSyncRequest, CrdtSyncResponse, NoteMetadataUpdate } from '$lib/types/note';

// Binary helpers for base64 encoding/decoding
export function uint8ArrayToBase64(bytes: Uint8Array): string {
  // Use browser-compatible approach
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

export function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export interface YjsDocManagerOptions {
  /** Callback when a document is updated locally */
  onLocalUpdate?: (noteId: string, update: Uint8Array) => void;
  /** Callback when document content changes */
  onContentChange?: (noteId: string, content: string) => void;
}

/**
 * Manages Yjs documents for all notes
 */
export class YjsDocManager {
  private docs: Map<string, Y.Doc> = new Map();
  private updateHandlers: Map<string, (update: Uint8Array, origin: any) => void> = new Map();
  private options: YjsDocManagerOptions;
  private pendingUpdates: Map<string, Uint8Array[]> = new Map();

  constructor(options: YjsDocManagerOptions = {}) {
    this.options = options;
  }

  /**
   * Get or create a Y.Doc for a note
   */
  getDoc(noteId: string): Y.Doc {
    let doc = this.docs.get(noteId);
    if (!doc) {
      doc = new Y.Doc();
      this.docs.set(noteId, doc);
      this.setupDocHandlers(noteId, doc);
    }
    return doc;
  }

  /**
   * Check if a document exists for a note
   */
  hasDoc(noteId: string): boolean {
    return this.docs.has(noteId);
  }

  /**
   * Get the Y.XmlFragment for the note content (used by TipTap)
   */
  getXmlFragment(noteId: string): Y.XmlFragment {
    const doc = this.getDoc(noteId);
    return doc.getXmlFragment('content');
  }

  /**
   * Get the text content from the Y.Doc (for plain text backup)
   */
  getTextContent(noteId: string): string {
    const doc = this.getDoc(noteId);
    const fragment = doc.getXmlFragment('content');
    return this.xmlFragmentToText(fragment);
  }

  /**
   * Initialize a document with existing content (for migration or new notes)
   */
  initializeWithContent(noteId: string, htmlContent: string): Y.Doc {
    const doc = this.getDoc(noteId);
    // Only initialize if the doc is empty
    const fragment = doc.getXmlFragment('content');
    if (fragment.length === 0 && htmlContent) {
      // The content will be set by TipTap when it binds to the document
      // We store it temporarily so TipTap can pick it up
      doc.getMap('meta').set('initialContent', htmlContent);
    }
    return doc;
  }

  /**
   * Populate the Y.Doc with HTML content programmatically
   * This is used during sync to ensure notes have CRDT content even without TipTap
   */
  populateFromHtml(noteId: string, htmlContent: string): Y.Doc {
    const doc = this.getDoc(noteId);
    const fragment = doc.getXmlFragment('content');
    
    // Only populate if empty and we have content, and only in browser
    if (!browser) {
      return doc; // Can't parse HTML without DOMParser
    }
    
    if (fragment.length === 0 && htmlContent && htmlContent.trim()) {
      doc.transact(() => {
        // Parse the HTML and create basic Y.XmlElement structure
        // This is a simplified version - TipTap will normalize when editing
        const parser = new DOMParser();
        const parsed = parser.parseFromString(htmlContent, 'text/html');
        const body = parsed.body;
        
        for (const child of Array.from(body.childNodes)) {
          this.appendNodeToFragment(fragment, child);
        }
      }, 'populate');
    }
    return doc;
  }

  private appendNodeToFragment(fragment: Y.XmlFragment, node: Node): void {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent || '';
      if (text) {
        const xmlText = new Y.XmlText();
        xmlText.insert(0, text);
        fragment.push([xmlText]);
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const element = node as Element;
      const tagName = element.tagName.toLowerCase();
      
      // Create XmlElement for block elements
      const xmlElement = new Y.XmlElement(tagName);
      
      // Copy attributes
      for (const attr of Array.from(element.attributes)) {
        xmlElement.setAttribute(attr.name, attr.value);
      }
      
      // Process children
      for (const child of Array.from(element.childNodes)) {
        this.appendNodeToXmlElement(xmlElement, child);
      }
      
      fragment.push([xmlElement]);
    }
  }

  private appendNodeToXmlElement(parent: Y.XmlElement, node: Node): void {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent || '';
      if (text) {
        const xmlText = new Y.XmlText();
        xmlText.insert(0, text);
        parent.push([xmlText]);
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const element = node as Element;
      const tagName = element.tagName.toLowerCase();
      const xmlElement = new Y.XmlElement(tagName);
      
      for (const attr of Array.from(element.attributes)) {
        xmlElement.setAttribute(attr.name, attr.value);
      }
      
      for (const child of Array.from(element.childNodes)) {
        this.appendNodeToXmlElement(xmlElement, child);
      }
      
      parent.push([xmlElement]);
    }
  }

  /**
   * Load document state from binary (from database)
   */
  loadState(noteId: string, state: Uint8Array): void {
    const doc = this.getDoc(noteId);
    Y.applyUpdate(doc, state, 'load');
  }

  /**
   * Load document state from base64 string
   */
  loadStateFromBase64(noteId: string, base64State: string): void {
    const state = base64ToUint8Array(base64State);
    this.loadState(noteId, state);
  }

  /**
   * Get the full state of a document as binary
   */
  getState(noteId: string): Uint8Array {
    const doc = this.getDoc(noteId);
    return Y.encodeStateAsUpdate(doc);
  }

  /**
   * Get the full state as base64 string
   */
  getStateAsBase64(noteId: string): string {
    return uint8ArrayToBase64(this.getState(noteId));
  }

  /**
   * Get the state vector for a document (used for diff sync)
   */
  getStateVector(noteId: string): Uint8Array {
    const doc = this.getDoc(noteId);
    return Y.encodeStateVector(doc);
  }

  /**
   * Get state vector as base64
   */
  getStateVectorAsBase64(noteId: string): string {
    return uint8ArrayToBase64(this.getStateVector(noteId));
  }

  /**
   * Apply a remote update to a document
   */
  applyUpdate(noteId: string, update: Uint8Array, origin: any = 'remote'): void {
    const doc = this.getDoc(noteId);
    Y.applyUpdate(doc, update, origin);
  }

  /**
   * Apply a base64-encoded remote update
   */
  applyUpdateFromBase64(noteId: string, base64Update: string, origin: any = 'remote'): void {
    const update = base64ToUint8Array(base64Update);
    this.applyUpdate(noteId, update, origin);
  }

  /**
   * Get the diff update needed by a remote peer
   * Given their state vector, return only the updates they're missing
   */
  getDiffUpdate(noteId: string, remoteStateVector: Uint8Array): Uint8Array {
    const doc = this.getDoc(noteId);
    return Y.encodeStateAsUpdate(doc, remoteStateVector);
  }

  /**
   * Get diff update with base64 encoding
   */
  getDiffUpdateAsBase64(noteId: string, remoteStateVectorBase64: string): string {
    const remoteStateVector = base64ToUint8Array(remoteStateVectorBase64);
    const diff = this.getDiffUpdate(noteId, remoteStateVector);
    return uint8ArrayToBase64(diff);
  }

  /**
   * Queue a pending update for later sync
   */
  queuePendingUpdate(noteId: string, update: Uint8Array): void {
    let pending = this.pendingUpdates.get(noteId);
    if (!pending) {
      pending = [];
      this.pendingUpdates.set(noteId, pending);
    }
    pending.push(update);
  }

  /**
   * Get and clear pending updates for a note
   */
  flushPendingUpdates(noteId: string): Uint8Array | null {
    const pending = this.pendingUpdates.get(noteId);
    if (!pending || pending.length === 0) {
      return null;
    }
    this.pendingUpdates.delete(noteId);
    
    // Merge all pending updates into one
    const doc = new Y.Doc();
    for (const update of pending) {
      Y.applyUpdate(doc, update);
    }
    return Y.encodeStateAsUpdate(doc);
  }

  /**
   * Check if a note has pending updates
   */
  hasPendingUpdates(noteId: string): boolean {
    const pending = this.pendingUpdates.get(noteId);
    return pending !== undefined && pending.length > 0;
  }

  /**
   * Get all note IDs with pending updates
   */
  getNotesWithPendingUpdates(): string[] {
    return Array.from(this.pendingUpdates.keys()).filter(id => this.hasPendingUpdates(id));
  }

  /**
   * Prepare a sync request to send to the server
   */
  prepareSyncRequest(noteIds: string[], metadata: NoteMetadataUpdate[]): CrdtSyncRequest {
    const stateVectors: Record<string, string> = {};
    const updates: Record<string, string> = {};

    for (const noteId of noteIds) {
      if (this.hasDoc(noteId)) {
        stateVectors[noteId] = this.getStateVectorAsBase64(noteId);
        
        // Include pending updates if any
        const pendingUpdate = this.flushPendingUpdates(noteId);
        if (pendingUpdate && pendingUpdate.length > 0) {
          updates[noteId] = uint8ArrayToBase64(pendingUpdate);
        }
      }
    }

    return {
      state_vectors: stateVectors,
      updates,
      metadata,
    };
  }

  /**
   * Apply a sync response from the server
   */
  applySyncResponse(response: CrdtSyncResponse): void {
    for (const [noteId, base64Update] of Object.entries(response.updates)) {
      if (base64Update && base64Update.length > 0) {
        this.applyUpdateFromBase64(noteId, base64Update, 'sync');
      }
    }
  }

  /**
   * Get CRDT state for persistence
   */
  getCrdtState(noteId: string): CrdtNoteState {
    return {
      id: noteId,
      ydoc_state: this.getState(noteId),
      state_vector: this.getStateVector(noteId),
      updated_at: new Date().toISOString(),
    };
  }

  /**
   * Destroy a document (cleanup)
   */
  destroyDoc(noteId: string): void {
    const doc = this.docs.get(noteId);
    if (doc) {
      const handler = this.updateHandlers.get(noteId);
      if (handler) {
        doc.off('update', handler);
        this.updateHandlers.delete(noteId);
      }
      doc.destroy();
      this.docs.delete(noteId);
    }
    this.pendingUpdates.delete(noteId);
  }

  /**
   * Destroy all documents
   */
  destroy(): void {
    for (const noteId of this.docs.keys()) {
      this.destroyDoc(noteId);
    }
  }

  /**
   * Get all managed document IDs
   */
  getAllDocIds(): string[] {
    return Array.from(this.docs.keys());
  }

  // Private methods

  private setupDocHandlers(noteId: string, doc: Y.Doc): void {
    const handler = (update: Uint8Array, origin: any) => {
      // Don't trigger for remote updates or loads
      if (origin === 'remote' || origin === 'sync' || origin === 'load') {
        return;
      }

      // Queue update for sync
      this.queuePendingUpdate(noteId, update);

      // Notify listeners
      this.options.onLocalUpdate?.(noteId, update);
    };

    doc.on('update', handler);
    this.updateHandlers.set(noteId, handler);

    // Also listen for content changes
    const fragment = doc.getXmlFragment('content');
    fragment.observe(() => {
      const content = this.xmlFragmentToText(fragment);
      this.options.onContentChange?.(noteId, content);
    });
  }

  private xmlFragmentToText(fragment: Y.XmlFragment): string {
    // Convert XmlFragment to HTML string
    // This is a simplified version; TipTap handles the actual rendering
    let result = '';
    fragment.forEach((item: Y.XmlText | Y.XmlElement) => {
      if (item instanceof Y.XmlText) {
        result += item.toString();
      } else if (item instanceof Y.XmlElement) {
        result += this.xmlElementToString(item);
      }
    });
    return result;
  }

  private xmlElementToString(element: Y.XmlElement): string {
    const tag = element.nodeName;
    const attrs = element.getAttributes();
    let attrString = '';
    for (const [key, value] of Object.entries(attrs)) {
      attrString += ` ${key}="${value}"`;
    }
    
    let content = '';
    element.forEach((child: Y.XmlText | Y.XmlElement) => {
      if (child instanceof Y.XmlText) {
        content += child.toString();
      } else if (child instanceof Y.XmlElement) {
        content += this.xmlElementToString(child);
      }
    });
    
    if (content) {
      return `<${tag}${attrString}>${content}</${tag}>`;
    }
    return `<${tag}${attrString} />`;
  }
}

// Singleton instance for app-wide use
let globalManager: YjsDocManager | null = null;

export function getYjsDocManager(options?: YjsDocManagerOptions): YjsDocManager {
  if (!globalManager) {
    globalManager = new YjsDocManager(options);
  } else if (options) {
    // If options provided and manager exists, merge the callbacks
    // This ensures callbacks are set even if manager was created elsewhere
    if (options.onLocalUpdate) {
      const existingUpdate = globalManager['options'].onLocalUpdate;
      globalManager['options'].onLocalUpdate = (noteId, update) => {
        existingUpdate?.(noteId, update);
        options.onLocalUpdate!(noteId, update);
      };
    }
    if (options.onContentChange) {
      const existingChange = globalManager['options'].onContentChange;
      globalManager['options'].onContentChange = (noteId, content) => {
        existingChange?.(noteId, content);
        options.onContentChange!(noteId, content);
      };
    }
  }
  return globalManager;
}

export function destroyYjsDocManager(): void {
  if (globalManager) {
    globalManager.destroy();
    globalManager = null;
  }
}
