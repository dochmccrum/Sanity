<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import type { Note } from '$lib/types/note';
  import type { Folder } from '$lib/api/folders';
  import CollaborativeEditor from '$lib/components/CollaborativeEditor.svelte';
  import SettingsModal from '$lib/components/SettingsModal.svelte';
  import { uploadImage } from '$lib/utils/imageUpload';
  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;

  let notesStore: any = $state(null);
  let foldersStore: any = $state(null);
  let settingsStore: any = $state(null);
  let initialized = $state(false);
  let showSettings = $state(false);
  let editingFolderId = $state<string | null>(null);
  let editingFolderName = $state('');
  let initError = $state<string | null>(null);
  let titleInput = $state<HTMLInputElement | null>(null);
  let folderInput = $state<HTMLInputElement | null>(null);
  let editorElement = $state<any>(null);
  let justCreatedNote = $state(false);
  let justCreatedFolder = $state(false);
  let showDeleteConfirm = $state(false);
  let noteToDelete = $state<Note | null>(null);
  let dontAskAgain = $state(false);
  let showDeleteFolderConfirm = $state(false);
  let folderToDelete = $state<string | null>(null);
  let leftSidebarCollapsed = $state(false);
  let rightSidebarCollapsed = $state(false);
  let leftSidebarWidth = $state(280);
  let rightSidebarWidth = $state(320);
  let isDraggingLeft = $state(false);
  let isDraggingRight = $state(false);
  let leftSidebarElement: HTMLElement | null = null;
  let rightSidebarElement: HTMLElement | null = null;
  let rafId: number | null = null;
  let targetLeftWidth: number | null = null;
  let targetRightWidth: number | null = null;
  let currentLeftWidth: number = 280;
  let currentRightWidth: number = 320;
  const smoothing = 0.3; // More smoothing for fluid feel
  let draggedNote: Note | null = $state(null);
  let dragOverFolder: string | null = $state(null);
  let expandedNotePreview = $state<string | null>(null);
  let syncInProgress = $state(false);
  let isMobile = $state(false);
  let activeMobilePane = $state<'folders' | 'notes' | 'editor'>('folders');

  // Auto-sync (Tauri only): debounce pushes after edits + periodic pulls.
  let autoSyncTimeout: ReturnType<typeof setTimeout> | null = null;
  let autoPullInterval: ReturnType<typeof setInterval> | null = null;
  let syncQueued = false;

  // Helper function to strip HTML tags for preview
  function stripHtml(html: string): string {
    const tmp = document.createElement('div');
    tmp.innerHTML = html;
    return tmp.textContent || tmp.innerText || '';
  }

  function checkMobile() {
    isMobile = window.innerWidth < 768;
    if (!isMobile) {
      // Reset sidebar states when going to desktop
      leftSidebarCollapsed = false;
      rightSidebarCollapsed = false;
    }
  }

  async function handleSyncNow() {
    if (!browser || !isTauri || syncInProgress) return;
    if (!settingsStore?.syncServerUrl) {
      throw new Error('Set a Server URL first');
    }

    const hasToken = !!localStorage.getItem('jwt');
    if (!hasToken) {
      throw new Error('Login first');
    }

    syncInProgress = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const baseUrl = String(settingsStore.syncServerUrl).trim().replace(/\/+$/, '');
      const token = localStorage.getItem('jwt')!;

      // 1) Sync folders first (still using REST for folders)
      const serverSince = localStorage.getItem('jfnotes_last_sync');
      const localSince = localStorage.getItem('jfnotes_last_local_sync');

      const localFolders: any[] = await invoke('get_folders_updated_since', {
        since: localSince || null,
      });

      // Get all local folder IDs so server can return any we're missing
      const allLocalFolders: any[] = await invoke('get_folders_updated_since', {
        since: null,
      });
      const knownFolderIds = allLocalFolders.map((f: any) => f.id);

      const foldersRes = await fetch(`${baseUrl}/api/sync/folders`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ 
          since: serverSince || undefined, 
          folders: localFolders,
          known_folder_ids: knownFolderIds,
        }),
      });

      if (!foldersRes.ok) {
        throw new Error(`Folder sync failed: ${foldersRes.status}`);
      }

      const foldersJson = (await foldersRes.json()) as { pulled: any[]; last_sync: string };
      console.log('[FolderSync] Pulled folders from server:', foldersJson.pulled.length, foldersJson.pulled);
      await invoke('apply_sync_folders', { folders: foldersJson.pulled });
      console.log('[FolderSync] Applied sync folders to local DB');

      // Reload folders immediately after applying sync
      await foldersStore?.loadFolders?.();

      // 2) Sync note CRDT states and metadata
      try {
        if (notesStore?.syncCrdtToServer) {
          // Pass the last server sync timestamp to optimize upload payload
          // If null, it will push everything (full sync)
          await notesStore.syncCrdtToServer(baseUrl, token, serverSince);
        }
      } catch (crdtErr) {
        console.warn('[Sync] CRDT sync failed, but folder sync succeeded:', crdtErr);
        // Don't throw - folder sync was successful
      }

      // Update sync timestamps
      localStorage.setItem('jfnotes_last_sync', new Date().toISOString());
      localStorage.setItem('jfnotes_last_local_sync', new Date().toISOString());
      settingsStore?.refreshLastSync?.();

      // Reload notes to reflect any server changes
      const selected = foldersStore?.selectedFolder;
      if (selected === 'uncategorised') {
        await notesStore?.loadNotes?.(undefined, true);
      } else if (selected && typeof selected !== 'string') {
        await notesStore?.loadNotes?.(selected.id);
      } else {
        await notesStore?.loadNotes?.();
      }
    } finally {
      syncInProgress = false;
    }
  }

  function scheduleAutoSync() {
    if (!browser || !isTauri) return;
    syncQueued = true;
    if (autoSyncTimeout) clearTimeout(autoSyncTimeout);
    autoSyncTimeout = setTimeout(() => {
      if (syncQueued) {
        syncQueued = false;
        void autoSync();
      }
    }, 1500);
  }

  async function autoSync() {
    if (!browser || !isTauri) return;
    if (syncInProgress) {
      // Coalesce further edits while a sync is running.
      syncQueued = true;
      return;
    }
    if (!settingsStore?.syncServerUrl) return;
    if (!localStorage.getItem('jwt')) return;

    try {
      await handleSyncNow();
    } catch (e) {
      // Background sync should never interrupt the editor.
      console.warn('Auto-sync failed:', e);
    }
  }

  function handleLeftResizeStart() {
    isDraggingLeft = true;
    if (leftSidebarElement) {
      leftSidebarElement.style.transition = 'none';
      currentLeftWidth = parseInt(leftSidebarElement.style.width) || leftSidebarWidth;
      targetLeftWidth = currentLeftWidth; // Set initial target
    }
    // Start continuous animation loop
    if (!rafId) {
      rafId = requestAnimationFrame(updateWidth);
    }
  }

  function handleRightResizeStart() {
    isDraggingRight = true;
    if (rightSidebarElement) {
      rightSidebarElement.style.transition = 'none';
      currentRightWidth = parseInt(rightSidebarElement.style.width) || rightSidebarWidth;
      targetRightWidth = currentRightWidth; // Set initial target
    }
    // Start continuous animation loop
    if (!rafId) {
      rafId = requestAnimationFrame(updateWidth);
    }
  }

  function updateWidth() {
    let shouldContinue = false;

    if (isDraggingLeft && targetLeftWidth !== null && leftSidebarElement) {
      // Linear interpolation for smooth following
      currentLeftWidth += (targetLeftWidth - currentLeftWidth) * smoothing;
      leftSidebarElement.style.width = `${currentLeftWidth}px`;
      shouldContinue = true;
    }

    if (isDraggingRight && targetRightWidth !== null && rightSidebarElement) {
      // Linear interpolation for smooth following
      currentRightWidth += (targetRightWidth - currentRightWidth) * smoothing;
      rightSidebarElement.style.width = `${currentRightWidth}px`;
      shouldContinue = true;
    }

    // Continue animating while dragging
    if (shouldContinue && (isDraggingLeft || isDraggingRight)) {
      rafId = requestAnimationFrame(updateWidth);
    } else {
      rafId = null;
    }
  }

  function handleResizeMouseMove(e: MouseEvent) {
    if (isDraggingLeft) {
      const newWidth = Math.max(200, Math.min(500, e.clientX));
      targetLeftWidth = newWidth;
    }
    if (isDraggingRight) {
      // Calculate width based on distance from the left sidebar edge
      const leftOffset = leftSidebarCollapsed ? 0 : leftSidebarWidth + 1; // +1 for left resize handle
      const mouseOffset = e.clientX - leftOffset;
      const newWidth = Math.max(200, Math.min(500, mouseOffset));
      targetRightWidth = newWidth;
    }
  }

  function handleResizeMouseUp() {
    // Immediately stop dragging flags to prevent any more frames
    const wasLeft = isDraggingLeft;
    const wasRight = isDraggingRight;
    isDraggingLeft = false;
    isDraggingRight = false;
    
    // Cancel any pending animation frame
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    
    // Update state to match final CSS values and restore transitions
    if (wasLeft && leftSidebarElement) {
      leftSidebarWidth = Math.round(currentLeftWidth);
      leftSidebarElement.style.width = `${leftSidebarWidth}px`;
      leftSidebarElement.style.transition = '';
    }
    if (wasRight && rightSidebarElement) {
      rightSidebarWidth = Math.round(currentRightWidth);
      rightSidebarElement.style.width = `${rightSidebarWidth}px`;
      rightSidebarElement.style.transition = '';
    }
    
    targetLeftWidth = null;
    targetRightWidth = null;
  }
  onMount(() => {
    // Setup event listeners for resize functionality
    if (typeof window !== 'undefined') {
      checkMobile();
      window.addEventListener('resize', () => {
        handleResizeMouseMove; // existing (needs proper wrap if complex, but here it is just a fn ref)
        checkMobile();
      });
      document.addEventListener('mousemove', handleResizeMouseMove);
      document.addEventListener('mouseup', handleResizeMouseUp);
    }

    const handleLocalChange = () => {
      scheduleAutoSync();
    };

    // Document-level drop handling removed to avoid duplicate inserts with Tauri drag/drop.
    
    // Listen for Tauri drag/drop events
    let unlistenFileDrop: (() => void) | null = null;
    let unlistenBackendDrop: (() => void) | null = null;
    const setupDragDrop = async () => {
      if (!browser || !isTauri) return;
      try {
        const [{ getCurrentWebview }, { invoke }] = await Promise.all([
          import('@tauri-apps/api/webview'),
          import('@tauri-apps/api/core')
        ]);
        const webview = getCurrentWebview();
        unlistenFileDrop = await webview.onDragDropEvent(async (event) => {
          const payload = event.payload;
          if (payload.type !== 'drop') return;

          const paths = payload.paths ?? [];
          console.log('Tauri file drop event:', paths);

          if (!notesStore?.selectedNote || !editorElement) {
            console.log('No note selected or editor not ready');
            return;
          }

          // Filter for image files
          const imageFiles = paths.filter((path: string) => 
            /\.(jpg|jpeg|png|gif|webp|bmp|svg)$/i.test(path)
          );

          if (imageFiles.length === 0) {
            console.log('No image files in drop');
            return;
          }

          console.log('Processing', imageFiles.length, 'image files');

          for (const filePath of imageFiles) {
            try {
              const result: any = await invoke('save_image_from_path', { path: filePath });
              console.log('Upload successful:', result);

              if (editorElement && result.uri) {
                editorElement.insertImage?.(result.uri);
              }
            } catch (error) {
              console.error('Failed to process dropped file:', filePath, error);
            }
          }
        });

        unlistenBackendDrop = null;
      } catch (error) {
        console.error('Failed to setup drag/drop listener:', error);
      }
    };

    const initializeStores = async () => {
      if (!browser) return;
      try {
        console.log('Initializing stores...');
        
        // Set a timeout to prevent infinite loading
        const timeout = setTimeout(() => {
          console.error('Store initialization timeout');
          initError = 'Failed to load app - timeout';
          initialized = true;
        }, 5000);
        
        const { createNotesStore } = await import('$lib/stores/notes.svelte');
        const { createFoldersStore } = await import('$lib/stores/folders.svelte');
        const { createSettingsStore } = await import('$lib/stores/settings.svelte');
        notesStore = createNotesStore();
        foldersStore = createFoldersStore();
        settingsStore = createSettingsStore();
        settingsStore.loadSettings();

        // Auto-sync wiring (Tauri only)
        if (isTauri) {
          const wrapMutation = (obj: any, key: string) => {
            const original = obj?.[key];
            if (typeof original !== 'function') return;
            obj[key] = async (...args: any[]) => {
              // Start debounce immediately so typing feels responsive.
              scheduleAutoSync();
              return original(...args);
            };
          };

          wrapMutation(notesStore, 'createNote');
          wrapMutation(notesStore, 'updateNote');
          wrapMutation(notesStore, 'deleteNote');
          wrapMutation(notesStore, 'moveNote');
          wrapMutation(foldersStore, 'createFolder');
          wrapMutation(foldersStore, 'updateFolder');
          wrapMutation(foldersStore, 'deleteFolder');

          if (autoPullInterval) clearInterval(autoPullInterval);
          autoPullInterval = setInterval(() => {
            void autoSync();
          }, 10000);

          window.addEventListener('jfnotes:local-change', handleLocalChange);
        } else {
          if (autoPullInterval) clearInterval(autoPullInterval);
          autoPullInterval = setInterval(() => {
            // Reload notes respecting current folder selection
            const selected = foldersStore?.selectedFolder;
            if (selected === 'uncategorised') {
              void notesStore.loadNotes(undefined, true);
            } else if (selected && typeof selected !== 'string') {
              void notesStore.loadNotes(selected.id);
            } else {
              void notesStore.loadNotes();
            }
            void foldersStore.loadFolders();
          }, 15000);
        }
        
        console.log('Loading data...');
        await Promise.all([
          notesStore.loadNotes(),
          foldersStore.loadFolders()
        ]);
        
        clearTimeout(timeout);
        initialized = true;
        console.log('Data loaded successfully');
      } catch (error) {
        console.error('Error initializing app:', error);
        initError = error instanceof Error ? error.message : 'Failed to initialize';
        // Set stores anyway so UI shows, but with errors
        if (!notesStore || !foldersStore) {
          const { createNotesStore } = await import('$lib/stores/notes.svelte');
          const { createFoldersStore } = await import('$lib/stores/folders.svelte');
          const { createSettingsStore } = await import('$lib/stores/settings.svelte');
          notesStore = createNotesStore();
          foldersStore = createFoldersStore();
          settingsStore = createSettingsStore();
        }
        initialized = true;
      }
    };

    void setupDragDrop();
    void initializeStores();
    
    // Return cleanup function
    return () => {
      if (unlistenFileDrop) {
        unlistenFileDrop();
      }
      if (unlistenBackendDrop) {
        unlistenBackendDrop();
      }
      if (typeof window !== 'undefined') {
        window.removeEventListener('resize', checkMobile);
        document.removeEventListener('mousemove', handleResizeMouseMove);
        document.removeEventListener('mouseup', handleResizeMouseUp);
      }

      if (autoPullInterval) {
        clearInterval(autoPullInterval);
        autoPullInterval = null;
      }
      if (autoSyncTimeout) {
        clearTimeout(autoSyncTimeout);
        autoSyncTimeout = null;
      }
      if (isTauri && typeof window !== 'undefined') {
        window.removeEventListener('jfnotes:local-change', handleLocalChange);
      }
    };
  });

  $effect(() => {
    if (!browser) return;
    const url = (isTauri ? settingsStore?.syncServerUrl : (settingsStore?.syncServerUrl || window.location.origin))?.trim();
    if (!url || !notesStore?.initWebSocketSync) return;
    notesStore.initWebSocketSync(url, () => localStorage.getItem('jwt'));
  });

  async function handleCreateNote() {
    if (!notesStore || !foldersStore) return;
    // When in uncategorised view or all notes, create without folder
    // When in a specific folder, create in that folder
    const selected = foldersStore.selectedFolder;
    const folderId = (selected && typeof selected !== 'string') ? selected.id : undefined;
    const note = await notesStore.createNote(folderId);
    if (note) {
      notesStore.selectNote(note);
      justCreatedNote = true;
      if (isMobile) activeMobilePane = 'editor';
    }
  }

  // Auto-focus and select title only when a NEW note is created (if setting is enabled)
  $effect(() => {
    if (justCreatedNote && notesStore?.selectedNote && settingsStore?.autoFocusTitleOnNewNote && titleInput) {
      setTimeout(() => {
        titleInput?.focus();
        titleInput?.select();
      }, 0);
      justCreatedNote = false;
    }
  });

  // Auto-focus and select folder name when a NEW folder is created
  $effect(() => {
    if (justCreatedFolder && editingFolderId && folderInput) {
      setTimeout(() => {
        folderInput?.focus();
        folderInput?.select();
      }, 0);
      justCreatedFolder = false;
    }
  });

  function handleTitleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && editorElement) {
      e.preventDefault();
      editorElement?.focus?.();
    }
  }

  async function handleCreateFolder() {
    if (!foldersStore) return;
    const parentId =
      foldersStore.selectedFolder && foldersStore.selectedFolder !== 'uncategorised'
        ? foldersStore.selectedFolder.id
        : null;
    const folder = await foldersStore.createFolder(parentId);
    if (folder) {
      editingFolderId = folder.id;
      editingFolderName = folder.name;
      justCreatedFolder = true;
    }
  }

  type FolderNode = Folder & { children: FolderNode[] };
  type FolderListItem = { folder: Folder; depth: number };

  function buildFolderTree(folders: Folder[]): FolderNode[] {
    const nodes = new Map<string, FolderNode>();
    for (const folder of folders) {
      nodes.set(folder.id, { ...folder, children: [] });
    }

    const roots: FolderNode[] = [];
    for (const node of nodes.values()) {
      const parentId = node.parent_id;
      if (parentId && nodes.has(parentId)) {
        nodes.get(parentId)!.children.push(node);
      } else {
        roots.push(node);
      }
    }

    const sortNodes = (list: FolderNode[]) => {
      list.sort((a, b) => a.name.localeCompare(b.name));
      for (const item of list) {
        if (item.children.length) sortNodes(item.children);
      }
    };
    sortNodes(roots);

    return roots;
  }

  function flattenFolderTree(nodes: FolderNode[], depth = 0, acc: FolderListItem[] = []) {
    for (const node of nodes) {
      acc.push({ folder: node, depth });
      if (node.children.length) {
        flattenFolderTree(node.children, depth + 1, acc);
      }
    }
    return acc;
  }

  let flatFolders = $state<FolderListItem[]>([]);
  $effect(() => {
    const allFolders = foldersStore?.folders ?? [];
    flatFolders = flattenFolderTree(buildFolderTree(allFolders));
  });

  function handleEditFolder(folder: Folder) {
    editingFolderId = folder.id;
    editingFolderName = folder.name;
  }

  async function handleSaveFolder() {
    if (!foldersStore) return;
    if (editingFolderId && editingFolderName.trim()) {
      const folder = foldersStore.folders.find((f: Folder) => f.id === editingFolderId);
      if (folder) {
        await foldersStore.updateFolder({
          id: folder.id,
          name: editingFolderName.trim(),
          parent_id: folder.parent_id
        });
      }
    }
    editingFolderId = null;
    editingFolderName = '';
  }

  async function handleDeleteFolder(folderId: string) {
    if (!foldersStore || !notesStore) return;
    folderToDelete = folderId;
    showDeleteFolderConfirm = true;
  }

  async function confirmDeleteFolder() {
    if (!foldersStore || !notesStore || !folderToDelete) return;
    
    // Delete all notes in this folder first
    const notesToDelete = notesStore.notes.filter((note: Note) => note.folder_id === folderToDelete);
    for (const note of notesToDelete) {
      await notesStore.deleteNote(note.id);
    }
    
    await foldersStore.deleteFolder(folderToDelete);
    if (foldersStore.selectedFolder?.id === folderToDelete) {
      // Reload all notes if we deleted the currently selected folder
      await notesStore.loadNotes();
    }
    showDeleteFolderConfirm = false;
    folderToDelete = null;
  }

  function cancelDeleteFolder() {
    showDeleteFolderConfirm = false;
    folderToDelete = null;
  }

  function handleSelectFolder(folder: Folder | null | 'uncategorised') {
    if (!foldersStore || !notesStore) return;
    if (folder === 'uncategorised') {
      foldersStore.selectedFolder = 'uncategorised';
      notesStore.selectNote(null);
      // Load notes without a folder
      notesStore.loadNotes(null, true); // true = only uncategorised
    } else {
      foldersStore.selectFolder(folder);
      notesStore.selectNote(null);
      // Reload notes filtered by the selected folder
      notesStore.loadNotes(folder?.id);
    }
    if (isMobile) activeMobilePane = 'notes';
  }

  function handleContentChange(value: string) {
    if (!notesStore) return;
    if (notesStore.selectedNote) {
      notesStore.updateNote({
        ...notesStore.selectedNote,
        content: value
      });
    }
  }

  // Debounced title save - save 500ms after user stops typing
  let titleSaveTimeout: ReturnType<typeof setTimeout> | null = null;
  let lastSavedTitle: string | null = null;
  let lastSavedNoteId: string | null = null;
  $effect(() => {
    const currentNote = notesStore?.selectedNote;
    if (!currentNote) {
      lastSavedTitle = null;
      lastSavedNoteId = null;
      return;
    }
    
    // If switching to a different note, update tracking without saving
    if (currentNote.id !== lastSavedNoteId) {
      lastSavedNoteId = currentNote.id;
      lastSavedTitle = currentNote.title;
      return;
    }
    
    // Only trigger save if title actually changed from what we last saw
    if (currentNote.title !== lastSavedTitle) {
      if (titleSaveTimeout) clearTimeout(titleSaveTimeout);
      titleSaveTimeout = setTimeout(() => {
        if (notesStore?.selectedNote && notesStore.selectedNote.id === lastSavedNoteId) {
          notesStore.updateNote(notesStore.selectedNote);
          lastSavedTitle = notesStore.selectedNote.title;
        }
      }, 500);
    }
  });

  async function handleDeleteNote() {
    if (!notesStore || !foldersStore) return;
    if (notesStore.selectedNote) {
      handleDeleteNoteClick(notesStore.selectedNote);
    }
  }

  function handleDeleteNoteClick(note: Note) {
    noteToDelete = note;
    if (settingsStore?.confirmBeforeDelete) {
      showDeleteConfirm = true;
    } else {
      confirmDeleteNote();
    }
  }

  async function confirmDeleteNote() {
    if (!noteToDelete || !notesStore) return;
    
    if (dontAskAgain && settingsStore) {
      settingsStore.confirmBeforeDelete = false;
    }
    
    await notesStore.deleteNote(noteToDelete.id);
    if (notesStore.selectedNote?.id === noteToDelete.id) {
      notesStore.selectNote(null);
    }
    showDeleteConfirm = false;
    noteToDelete = null;
    dontAskAgain = false;
  }

  function cancelDelete() {
    showDeleteConfirm = false;
    noteToDelete = null;
    dontAskAgain = false;
  }

  // Drag and drop for notes
  function handleNoteDragStart(event: DragEvent, note: Note) {
    draggedNote = note;
    // Create a smaller drag image
    if (event.dataTransfer && event.currentTarget) {
      const target = event.currentTarget as HTMLElement;
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/plain', note.id);
      
      // Calculate offset based on where user clicked
      const rect = target.getBoundingClientRect();
      const scale = 0.5;
      const offsetX = (event.clientX - rect.left) * scale;
      const offsetY = (event.clientY - rect.top) * scale;
      
      // Create a scaled-down clone
      const clone = target.cloneNode(true) as HTMLElement;
      clone.style.position = 'absolute';
      clone.style.top = '-1000px';
      clone.style.width = target.offsetWidth + 'px';
      clone.style.transform = `scale(${scale})`;
      clone.style.transformOrigin = 'top left';
      clone.style.opacity = '0.7';
      clone.style.pointerEvents = 'none';
      document.body.appendChild(clone);
      
      event.dataTransfer.setDragImage(clone, offsetX, offsetY);
      
      // Clean up the clone after a short delay
      setTimeout(() => {
        document.body.removeChild(clone);
      }, 0);
    }
  }

  function handleNoteDragEnd() {
    draggedNote = null;
    dragOverFolder = null;
  }

  function handleFolderDragOver(event: DragEvent, folderId: string | null) {
    // Only handle if we're dragging a note, not an external file
    if (!draggedNote) return;
    
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
    dragOverFolder = folderId || 'all-notes';
  }

  function handleFolderDragLeave(event: DragEvent) {
    // Only clear highlight if leaving to a non-child element
    const target = event.currentTarget as HTMLElement;
    const relatedTarget = event.relatedTarget as HTMLElement;
    if (!relatedTarget || !target.contains(relatedTarget)) {
      dragOverFolder = null;
    }
  }

  async function handleFolderDrop(event: DragEvent, folderId: string | null) {
    // Only handle if we're dragging a note, not an external file
    if (!draggedNote) return;
    
    event.preventDefault();
    dragOverFolder = null;
    
    if (!notesStore) return;
    
    // Update the note's folder
    await notesStore.moveNote(draggedNote.id, folderId);
    
    // Reload notes for current view
    const selected = foldersStore?.selectedFolder;
    if (selected === 'uncategorised') {
      await notesStore.loadNotes(undefined, true);
    } else if (selected && typeof selected !== 'string') {
      await notesStore.loadNotes(selected.id);
    } else {
      await notesStore.loadNotes();
    }
    
    draggedNote = null;
  }
</script>

{#if !initialized}
  <div class="flex h-full items-center justify-center bg-gray-50">
    <div class="text-center">
      {#if initError}
        <div class="bg-red-50 border border-red-200 rounded-lg p-6 max-w-md">
          <svg class="w-12 h-12 mx-auto text-red-500 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
          </svg>
          <p class="text-red-700 font-medium mb-2">Error Loading App</p>
          <p class="text-red-600 text-sm">{initError}</p>
          <button class="btn btn-primary mt-4" onclick={() => window.location.reload()}>
            Reload
          </button>
        </div>
      {:else}
        <div class="inline-block w-12 h-12 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin mb-4"></div>
        <p class="text-gray-600">Loading Sanity...</p>
        <p class="text-gray-400 text-sm mt-2">This should only take a moment</p>
      {/if}
    </div>
  </div>
{:else}
<div class="flex h-full overflow-hidden bg-gray-50 flex-col md:flex-row" class:cursor-col-resize={isDraggingLeft || isDraggingRight}>
  <!-- Left Sidebar (Folders) -->
  <aside 
    bind:this={leftSidebarElement} 
    class="border-r border-gray-200 bg-white flex flex-col {isDraggingLeft ? '' : 'transition-all'} {leftSidebarCollapsed ? 'w-0 -ml-px' : ''} overflow-hidden" 
    style="width: {isMobile ? (activeMobilePane === 'folders' ? '100%' : '0px') : (leftSidebarCollapsed ? '0px' : leftSidebarWidth + 'px')}"
    class:hidden={isMobile && activeMobilePane !== 'folders'}
  >
    <div class="p-4 border-b border-gray-200">
      <div class="flex items-center justify-between">
        <div class="flex gap-2">
          <button class="p-2 hover:bg-gray-100 rounded-lg transition-colors text-gray-600" onclick={handleCreateNote} title="New Note">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>
            </svg>
          </button>
          <button class="p-2 hover:bg-gray-100 rounded-lg transition-colors text-gray-600" onclick={handleCreateFolder} title="New Folder">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
            </svg>
          </button>
        </div>
        <div class="flex gap-2">
          <button
            class="p-2 hover:bg-gray-100 rounded-lg transition-colors"
            onclick={() => (showSettings = true)}
            title="Settings"
          >
            <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
        {#if !isMobile}
          <button
              class="p-2 hover:bg-gray-100 rounded-lg transition-colors"
              onclick={() => (leftSidebarCollapsed = !leftSidebarCollapsed)}
              title={leftSidebarCollapsed ? "Expand" : "Collapse"}
            >
              <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={leftSidebarCollapsed ? "M9 5l7 7-7 7" : "M15 19l-7-7 7-7"} />
              </svg>
            </button>
        {/if}
      </div>
    </div>
   </div>

    <!-- Folders Section -->
    <div class="flex-1 overflow-y-auto">
      <div class="px-4 py-2 space-y-1">
        {#if settingsStore?.showAllNotesFolder}
          <button 
            class="w-full text-left px-3 py-2 rounded-lg hover:bg-gray-100 transition-colors {!foldersStore.selectedFolder && foldersStore.selectedFolder !== 'uncategorised' ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'} {dragOverFolder === 'all-notes' ? 'ring-2 ring-indigo-400 bg-indigo-50' : ''}"
            onclick={() => handleSelectFolder(null)}
            ondragover={(e) => handleFolderDragOver(e, null)}
            ondragleave={(e) => handleFolderDragLeave(e)}
            ondrop={(e) => handleFolderDrop(e, null)}
            ondragenter={(e) => handleFolderDragOver(e, null)}
          >
            <svg class="w-4 h-4 inline-block mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
            </svg>
            All Notes
          </button>
        {/if}
        
        {#if settingsStore?.showUncategorisedFolder}
          <button 
            class="w-full text-left px-3 py-2 rounded-lg hover:bg-gray-100 transition-colors {foldersStore.selectedFolder === 'uncategorised' ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'} {dragOverFolder === 'uncategorised' ? 'ring-2 ring-indigo-400 bg-indigo-50' : ''}"
            onclick={() => handleSelectFolder('uncategorised')}
            ondragover={(e) => handleFolderDragOver(e, null)}
            ondragleave={(e) => handleFolderDragLeave(e)}
            ondrop={(e) => handleFolderDrop(e, null)}
            ondragenter={(e) => handleFolderDragOver(e, null)}
          >
            <svg class="w-4 h-4 inline-block mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m5 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
            </svg>
            Uncategorised
          </button>
        {/if}
      </div>

      {#if !foldersStore}
        <div class="text-center text-gray-500 py-4">
          <div class="inline-block w-6 h-6 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin"></div>
        </div>
      {:else if foldersStore.loading && flatFolders.length === 0}
        <div class="text-center text-gray-500 py-4">
          <div class="inline-block w-6 h-6 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin"></div>
        </div>
      {:else if flatFolders.length > 0}
        <div class="px-4 pb-4 space-y-1 transition-opacity duration-200" class:opacity-50={foldersStore.loading}>
          {#each flatFolders as item}
            {@const folder = item.folder}
            <div class="group relative">
              {#if editingFolderId === folder.id}
                <div class="flex items-center gap-2" style={`padding-left: ${12 + item.depth * 16}px; padding-right: 12px;`}>
                  <input
                    bind:this={folderInput}
                    type="text"
                    bind:value={editingFolderName}
                    class="input flex-1 text-sm"
                    onkeydown={(e) => e.key === 'Enter' && handleSaveFolder()}
                    onblur={handleSaveFolder}
                    autofocus
                  />
                </div>
              {:else}
                <div
                  class="w-full text-left py-2 rounded-lg hover:bg-gray-100 transition-colors flex items-center justify-between cursor-pointer {foldersStore.selectedFolder?.id === folder.id ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'} {dragOverFolder === folder.id ? 'ring-2 ring-indigo-400 bg-indigo-50' : ''}"
                  style={`padding-left: ${12 + item.depth * 16}px; padding-right: 12px;`}
                  onclick={() => handleSelectFolder(folder)}
                  ondragover={(e) => handleFolderDragOver(e, folder.id)}
                  ondragleave={(e) => handleFolderDragLeave(e)}
                  ondrop={(e) => handleFolderDrop(e, folder.id)}
                  ondragenter={(e) => handleFolderDragOver(e, folder.id)}
                  role="button"
                  tabindex="0"
                >
                  <span class="flex items-center pointer-events-none">
                    <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
                    </svg>
                    <span class="truncate">{folder.name}</span>
                  </span>
                  <div class="flex gap-1 pointer-events-auto transition-opacity" class:opacity-0={!isMobile} class:group-hover:opacity-100={!isMobile}>
                    <div
                      class="p-1 hover:bg-gray-200 rounded cursor-pointer"
                      onclick={(e) => { e.stopPropagation(); handleEditFolder(folder); }}
                      role="button"
                      tabindex="0"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
                      </svg>
                    </div>
                    <div
                      class="p-1 hover:bg-red-100 rounded text-red-600 cursor-pointer"
                      onclick={(e) => { e.stopPropagation(); handleDeleteFolder(folder.id); }}
                      role="button"
                      tabindex="0"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                      </svg>
                    </div>
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </aside>

  <!-- Left Sidebar Expand Button (when collapsed) -->
  {#if leftSidebarCollapsed && !isMobile}
    <button
      class="w-8 h-12 bg-white border border-gray-200 rounded-r-lg flex items-center justify-center hover:bg-indigo-50 transition-colors shadow-sm"
      onclick={() => (leftSidebarCollapsed = false)}
      title="Expand folders sidebar"
      style="position: absolute; left: 0; top: 50%; transform: translateY(-50%); z-index: 10;"
    >
      <svg class="w-4 h-4 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
      </svg>
    </button>
  {/if}

  <!-- Left Sidebar Resize Handle -->
  {#if !leftSidebarCollapsed && !isMobile}
    <div
      class="w-1 bg-gray-200 hover:bg-indigo-500 cursor-col-resize transition-colors {isDraggingLeft ? 'bg-indigo-500' : ''}"
      onmousedown={handleLeftResizeStart}
      role="separator"
      aria-orientation="vertical"
    />
  {/if}

  <!-- Right Sidebar (Notes) -->
  <aside 
    bind:this={rightSidebarElement} 
    class="border-r border-gray-200 bg-white flex flex-col {isDraggingRight ? '' : 'transition-all'} {rightSidebarCollapsed ? 'w-0 -mr-px' : ''} overflow-hidden" 
    style="width: {isMobile ? (activeMobilePane === 'notes' ? '100%' : '0px') : (rightSidebarCollapsed ? '0px' : rightSidebarWidth + 'px')}"
    class:hidden={isMobile && activeMobilePane !== 'notes'}
  >
    <div class="p-4 border-b border-gray-200">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold text-gray-900">Notes</h2>
        <div class="flex gap-2">
        <button
          class="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          onclick={() => (rightSidebarCollapsed = !rightSidebarCollapsed)}
          title={rightSidebarCollapsed ? "Expand" : "Collapse"}
        >
          <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={rightSidebarCollapsed ? "M15 19l-7-7 7-7" : "M9 5l7 7-7 7"} />
          </svg>
        </button>
        <button
          class="p-2 hover:bg-gray-100 rounded-lg transition-colors"
          onclick={() => handleCreateNote()}
          title="New note"
        >
          <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v14m7-7H5" />
          </svg>
        </button>
        </div>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-4">
      {#if !notesStore}
        <div class="text-center text-gray-500 py-8">
          <div class="inline-block w-8 h-8 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin"></div>
          <p class="mt-2">Initializing...</p>
        </div>
      {:else if notesStore.loading && notesStore.notes.length === 0}
        <div class="text-center text-gray-500 py-8">
          <div class="inline-block w-8 h-8 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin"></div>
          <p class="mt-2">Loading notes...</p>
        </div>
      {:else if notesStore.error}
        <div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-700">
          <p class="font-medium">Error</p>
          <p class="text-sm mt-1">{notesStore.error}</p>
        </div>
      {:else if notesStore.notes.length === 0}
        <div class="text-center text-gray-500 py-8">
          <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
          </svg>
          <p class="text-lg font-medium">No notes yet</p>
          <p class="text-sm mt-1">Create your first note to get started</p>
        </div>
      {:else}
        <div class="space-y-2 transition-opacity duration-200" class:opacity-50={notesStore.loading}>
          {#each notesStore.notes as note}
            <div class="group relative">
              <button
                class="w-full text-left p-4 rounded-lg border border-gray-200 hover:border-indigo-400 hover:bg-indigo-50 transition-all {notesStore.selectedNote?.id === note.id ? 'bg-indigo-50 border-indigo-400' : 'bg-white'} {draggedNote?.id === note.id ? 'opacity-50' : ''}"
                onclick={() => {
                  notesStore.selectNote(note);
                  if (isMobile) activeMobilePane = 'editor';
                }}
                draggable="true"
                ondragstart={(e) => handleNoteDragStart(e, note)}
                ondragend={handleNoteDragEnd}
              >
                <h3 class="font-medium text-gray-900 truncate">
                  {note.title || 'Untitled Note'}
                </h3>
                {#if settingsStore?.showNotePreviews}
                  {#if expandedNotePreview === note.id}
                    <div class="text-sm text-gray-700 mt-2 p-2 bg-gray-50 rounded border border-gray-200 max-h-32 overflow-y-auto break-words whitespace-normal">
                      {note.content ? stripHtml(note.content) : 'No content'}
                    </div>
                  {:else}
                    <p class="text-sm text-gray-500 mt-1 line-clamp-2 cursor-pointer hover:text-gray-600" onclick={(e) => {
                      e.stopPropagation();
                      expandedNotePreview = note.id;
                    }}>
                      {note.content ? stripHtml(note.content).substring(0, 100) : 'Open to view content'}
                    </p>
                  {/if}
                  {#if expandedNotePreview === note.id && note.content}
                    <button 
                      class="text-xs text-gray-400 hover:text-gray-600 mt-2"
                      onclick={(e) => {
                        e.stopPropagation();
                        expandedNotePreview = null;
                      }}
                    >
                      Show less
                    </button>
                  {/if}
                {/if}
                <div class="flex items-center justify-between mt-2">
                  <span class="text-xs text-gray-400">
                    {new Date(note.updated_at).toLocaleDateString()}
                  </span>
                  <div class="flex items-center gap-2">
                    {#if !foldersStore.selectedFolder && note.folder_id}
                      {@const noteFolder = foldersStore.folders.find((f: Folder) => f.id === note.folder_id)}
                      {#if noteFolder}
                        <span class="px-2 py-1 bg-indigo-100 text-indigo-700 text-xs rounded flex items-center gap-1">
                          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
                          </svg>
                          {noteFolder.name}
                        </span>
                      {/if}
                    {/if}
                    {#if note.is_canvas}
                      <span class="px-2 py-1 bg-purple-100 text-purple-700 text-xs rounded">Canvas</span>
                    {/if}
                  </div>
                </div>
              </button>
              <button
                class="absolute top-2 right-2 p-2 rounded-lg bg-white/90 hover:bg-red-50 transition-opacity shadow-sm"
                class:opacity-0={!isMobile}
                class:group-hover:opacity-100={!isMobile}
                onclick={(e) => {
                  e.stopPropagation();
                  handleDeleteNoteClick(note);
                }}
                aria-label="Delete note"
              >
                <svg class="w-4 h-4 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                </svg>
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </aside>

  <!-- Right Sidebar Expand Button (when collapsed) -->
  {#if rightSidebarCollapsed && !isMobile}
    <button
      class="w-8 h-12 bg-white border border-gray-200 rounded-l-lg flex items-center justify-center hover:bg-indigo-50 transition-colors shadow-sm"
      onclick={() => (rightSidebarCollapsed = false)}
      title="Expand notes sidebar"
      style="position: absolute; left: {leftSidebarCollapsed ? 0 : leftSidebarWidth + 1}px; top: 50%; transform: translateY(-50%); z-index: 10;"
    >
      <svg class="w-4 h-4 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
      </svg>
    </button>
  {/if}

  <!-- Right Sidebar Resize Handle -->
  {#if !rightSidebarCollapsed && !isMobile}
    <div
      class="w-1 bg-gray-200 hover:bg-indigo-500 cursor-col-resize transition-colors {isDraggingRight ? 'bg-indigo-500' : ''}"
      onmousedown={handleRightResizeStart}
      role="separator"
      aria-orientation="vertical"
    />
  {/if}

  <!-- Editor Area -->
  <main 
    class="flex-1 flex flex-col bg-white overflow-hidden"
    class:hidden={isMobile && activeMobilePane !== 'editor'}
    ondragover={(e) => {
      // Allow file drops in editor area
      if (e.dataTransfer?.types.includes('Files')) {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'copy';
      }
    }}
  >
    {#if notesStore.selectedNote}
      <div class="border-b border-gray-200 p-4 flex items-center gap-2">
        {#if isMobile}
          <button class="p-2 -ml-2 text-gray-500 hover:bg-gray-100 rounded-lg" onclick={() => activeMobilePane = 'notes'}>
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
        {/if}
        <input
          bind:this={titleInput}
          type="text"
          class="flex-1 text-xl md:text-2xl font-bold bg-transparent border-none outline-none"
          placeholder="Note title..."
          bind:value={notesStore.selectedNote.title}
          onkeydown={handleTitleKeydown}
        />
      </div>

      <div class="flex-1 overflow-hidden">
        <CollaborativeEditor
          bind:this={editorElement}
          noteId={notesStore.selectedNote.id}
          ydoc={notesStore.getYjsDoc(notesStore.selectedNote.id)}
          initialContent={notesStore.selectedNote.content}
          enableAutoComplete={settingsStore?.enableAutoComplete ?? true}
        />
      </div>
    {:else}
      <div class="flex-1 flex items-center justify-center text-gray-400 p-4">
        <div class="text-center">
          <svg class="w-16 h-16 md:w-24 md:h-24 mx-auto mb-4 text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
          </svg>
          <p class="text-lg md:text-xl font-medium">No note selected</p>
          <p class="mt-2 text-sm md:text-base">Select a note or create a new one</p>
          {#if isMobile}
            <button class="btn btn-primary mt-6" onclick={() => activeMobilePane = 'folders'}>
              View Folders
            </button>
          {/if}
        </div>
      </div>
    {/if}
  </main>

  <!-- Mobile Bottom Navigation -->
  {#if isMobile}
    <nav class="flex border-t border-gray-200 bg-white shrink-0" style="padding-bottom: env(safe-area-inset-bottom); height: calc(4rem + env(safe-area-inset-bottom));">
      <button 
        class="flex-1 flex flex-col items-center justify-center gap-1 {activeMobilePane === 'folders' ? 'text-indigo-600' : 'text-gray-500'}"
        onclick={() => activeMobilePane = 'folders'}
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
        </svg>
        <span class="text-[10px] font-medium">Folders</span>
      </button>
      <button 
        class="flex-1 flex flex-col items-center justify-center gap-1 {activeMobilePane === 'notes' ? 'text-indigo-600' : 'text-gray-500'}"
        onclick={() => activeMobilePane = 'notes'}
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
        </svg>
        <span class="text-[10px] font-medium">Notes</span>
      </button>
      <button 
        class="flex-1 flex flex-col items-center justify-center gap-1 {activeMobilePane === 'editor' ? 'text-indigo-600' : 'text-gray-500'}"
        onclick={() => activeMobilePane = 'editor'}
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
        </svg>
        <span class="text-[10px] font-medium">Editor</span>
      </button>
    </nav>
  {/if}
</div>
{/if}

<SettingsModal bind:open={showSettings} settings={settingsStore} isTauri={isTauri} onSync={handleSyncNow} />

<!-- Delete Confirmation Modal -->
{#if showDeleteConfirm}
  <div class="fixed inset-0 flex items-center justify-center z-50" style="background-color: rgba(0, 0, 0, 0.1);" onmousedown={(e) => e.target === e.currentTarget && (showDeleteConfirm = false)} transition:fade={{ duration: 200 }}>
    <div class="bg-white rounded-lg shadow-xl max-w-md w-full mx-4" transition:scale={{ duration: 200, start: 0.95 }}>
      <div class="p-6">
        <div class="flex items-center gap-3 mb-4">
          <div class="flex-shrink-0 w-12 h-12 rounded-full bg-red-100 flex items-center justify-center">
            <svg class="w-6 h-6 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
            </svg>
          </div>
          <div>
            <h3 class="text-lg font-semibold text-gray-900">Delete Note?</h3>
            <p class="text-sm text-gray-500 mt-1">
              Are you sure you want to delete "{noteToDelete?.title || 'Untitled Note'}"? This action cannot be undone.
            </p>
          </div>
        </div>

        <div class="flex items-center gap-2 mb-6 px-2">
          <input
            type="checkbox"
            id="dont-ask-again"
            bind:checked={dontAskAgain}
            class="w-4 h-4 text-indigo-600 border-gray-300 rounded focus:ring-indigo-500"
          />
          <label for="dont-ask-again" class="text-sm text-gray-700 cursor-pointer">
            Don't ask me again
          </label>
        </div>

        <div class="flex gap-3 justify-end">
          <button
            class="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
            onclick={cancelDelete}
          >
            Cancel
          </button>
          <button
            class="px-4 py-2 text-white bg-red-600 rounded-lg hover:bg-red-700 transition-colors"
            onclick={confirmDeleteNote}
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
{#if showDeleteFolderConfirm}
  <div class="fixed inset-0 flex items-center justify-center z-50" style="background-color: rgba(0, 0, 0, 0.1);" onmousedown={(e) => e.target === e.currentTarget && (showDeleteFolderConfirm = false)} transition:fade={{ duration: 200 }}>
    <div class="bg-white rounded-lg shadow-xl max-w-md w-full mx-4" transition:scale={{ duration: 200, start: 0.95 }}>
      <div class="p-6">
        <div class="flex items-center gap-3 mb-4">
          <div class="flex-shrink-0 w-12 h-12 rounded-full bg-red-100 flex items-center justify-center">
            <svg class="w-6 h-6 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
            </svg>
          </div>
          <div>
            <h3 class="text-lg font-semibold text-gray-900">Delete Folder?</h3>
            <p class="text-sm text-gray-500 mt-1">
              Are you sure you want to delete "{foldersStore.folders.find((f: Folder) => f.id === folderToDelete)?.name || 'this folder'}"? All notes in this folder will also be deleted.
            </p>
          </div>
        </div>

        <div class="flex gap-3 justify-end">
          <button
            class="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
            onclick={cancelDeleteFolder}
          >
            Cancel
          </button>
          <button
            class="px-4 py-2 text-white bg-red-600 rounded-lg hover:bg-red-700 transition-colors"
            onclick={confirmDeleteFolder}
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}