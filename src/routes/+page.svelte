<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import type { Note } from '$lib/types/note';
  import type { Folder } from '$lib/api/folders';
  import CollaborativeEditor from '$lib/components/CollaborativeEditor.svelte';
  import SettingsModal from '$lib/components/SettingsModal.svelte';
  import { uploadImage } from '$lib/utils/imageUpload';
  import { createNotesStore } from '$lib/stores/notes.svelte';
  import { createFoldersStore } from '$lib/stores/folders.svelte';
  import { createSettingsStore } from '$lib/stores/settings.svelte';
  
  const isTauri = typeof window !== 'undefined' && (
    (window as any).__TAURI__ !== undefined || 
    (window as any).__TAURI_IPC__ !== undefined || 
    (window as any).__TAURI_INTERNALS__ !== undefined
  );

  let notesStore = createNotesStore();
  let foldersStore = createFoldersStore();
  let settingsStore = createSettingsStore();
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
  let draggedFolder: Folder | null = $state(null);
  let dragOverFolder: string | null = $state(null);
  let expandedNotePreview = $state<string | null>(null);
  let syncInProgress = $state(false);
  let isMobile = $state(false);
  let activeMobilePane = $state<'folders' | 'notes' | 'editor'>('folders');
  let showEditorMenu = $state(false);
  let editorMenuButton = $state<HTMLButtonElement | null>(null);
  let editorMenuContainer = $state<HTMLDivElement | null>(null);
  let showExportSuccess = $state(false);
  let exportedPath = $state('');
  let exportStatus = $state('');
  
  // Single Sidebar Mode state
  let expandedFolders = $state<Set<string>>(new Set());
  let selectedFolderIds = $state<Set<string>>(new Set());
  let lastSelectedFolderId = $state<string | null>(null);
  
  // Note multi-selection state
  let selectedNoteIds = $state<Set<string>>(new Set());
  let lastSelectedNoteId = $state<string | null>(null);
  
  function matchesShortcut(e: KeyboardEvent, shortcut: string): boolean {
    if (!shortcut) return false;
    const parts = shortcut.split('+').map(p => p.trim().toLowerCase());
    const key = parts.pop();
    
    const meta = parts.includes('control') || parts.includes('ctrl') || parts.includes('meta') || parts.includes('cmd');
    const alt = parts.includes('alt');
    const shift = parts.includes('shift');
    
    const eMeta = e.ctrlKey || e.metaKey;
    const eAlt = e.altKey;
    const eShift = e.shiftKey;
    
    return e.key.toLowerCase() === key && meta === eMeta && alt === eAlt && shift === eShift;
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    const isInput = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || (e.target as HTMLElement).isContentEditable;

    if (e.key === 'Delete' || e.key === 'Backspace') {
       if (isInput) return;

       if (selectedFolderIds.size > 0) {
           handleDeleteFolders();
       } else if (selectedNoteIds.size > 0) {
           handleDeleteSelectedNotes();
       } else if (notesStore?.selectedNote) {
            handleDeleteNoteClick(notesStore.selectedNote);
       }
       return;
    }

    if (!settingsStore?.shortcuts) return;
    const { shortcuts } = settingsStore;

    if (matchesShortcut(e, shortcuts.toggleSidebar)) {
      e.preventDefault();
      if (settingsStore.singleSidebarMode) {
        leftSidebarCollapsed = !leftSidebarCollapsed;
      } else {
        // Toggle both or intelligently? Let's toggle both for "B" shortcut style
        const target = !(leftSidebarCollapsed && rightSidebarCollapsed);
        leftSidebarCollapsed = target;
        rightSidebarCollapsed = target;
      }
    } else if (matchesShortcut(e, shortcuts.search)) {
      e.preventDefault();
      handleSearchOpen();
    } else if (matchesShortcut(e, shortcuts.newNote)) {
      e.preventDefault();
      handleCreateNote();
    } else if (matchesShortcut(e, shortcuts.newFolder)) {
      e.preventDefault();
      handleCreateFolder();
    } else if (matchesShortcut(e, shortcuts.exportPdf)) {
      e.preventDefault();
      handleExportPdf();
    }
  }

  async function handleDeleteFolders() {
    if (selectedFolderIds.size === 0) return;
    if (settingsStore?.confirmFolderDelete) {
        showDeleteFolderConfirm = true;
    } else {
        confirmDeleteFolder();
    }
  }

  async function handleDeleteSelectedNotes() {
    if (selectedNoteIds.size === 0) return;
    if (settingsStore?.confirmNoteDelete) {
        showDeleteConfirm = true;
    } else {
        confirmDeleteNote();
    }
  }

  $effect(() => {
      if (typeof window !== 'undefined') {
          window.addEventListener('keydown', handleWindowKeydown);
          return () => window.removeEventListener('keydown', handleWindowKeydown);
      }
  });

  function toggleFolder(folderId: string) {
    const next = new Set(expandedFolders);
    if (next.has(folderId)) {
      next.delete(folderId);
    } else {
      next.add(folderId);
    }
    expandedFolders = next;
    console.log('Toggled folder:', folderId, 'expanded:', next.has(folderId));
  }
  
  $effect(() => {
    // When switching to single sidebar mode, ensure all notes are loaded
    if (settingsStore?.singleSidebarMode && notesStore) {
       // Only trigger if we aren't already loading or have all notes
       // But how do we know if we have "all"? passing undefined to loadNotes gets all.
       // We can just call it.
       // Check if current notes list logic is "filtered".
       // If we toggle the setting, we want to refresh.
       notesStore.loadNotes();
    }
  });

  // Search state
  let showSearch = $state(false);
  let searchQuery = $state('');
  let searchInput = $state<HTMLInputElement | null>(null);
  let searchResults = $state<Note[]>([]);
  let allNotesCache = $state<Note[]>([]);

  // Auto-focus search input when opened
  $effect(() => {
    if (showSearch && searchInput) {
      setTimeout(() => {
        searchInput?.focus();
      }, 50);
    }
  });

  // Fast search implementation - builds index on first search, then filters instantly
  function performSearch(query: string) {
    if (!query.trim()) {
      searchResults = [];
      return;
    }
    
    const lowerQuery = query.toLowerCase();
    const terms = lowerQuery.split(/\s+/).filter(t => t.length > 0);
    
    // Search through all notes (not just current folder)
    const notesToSearch = allNotesCache.length > 0 ? allNotesCache : (notesStore?.notes || []);
    
    searchResults = notesToSearch.filter((note: Note) => {
      if (note.is_deleted) return false;
      
      const titleLower = (note.title || '').toLowerCase();
      const contentText = stripHtml(note.content || '').toLowerCase();
      
      // All search terms must match either title or content
      return terms.every(term => 
        titleLower.includes(term) || contentText.includes(term)
      );
    }).slice(0, 50); // Limit results for performance
  }

  // Load all notes for search (refreshes cache each time search opens for fresh content)
  async function loadAllNotesForSearch() {
    try {
      if (isTauri) {
        const { invoke } = await import('@tauri-apps/api/core');
        // Use get_notes_updated_since with null to get ALL notes WITH content
        // (get_notes returns NoteSummary which doesn't include content)
        allNotesCache = await invoke('get_notes_updated_since', { since: null });
      } else {
        // For web, use all notes from the store
        allNotesCache = notesStore?.notes || [];
      }
    } catch (err) {
      console.error('Failed to load notes for search:', err);
      allNotesCache = notesStore?.notes || [];
    }
  }

  function handleSearchOpen() {
    showSearch = true;
    // Always refresh notes cache to get latest content
    loadAllNotesForSearch();
    // Focus will be handled by the bind:this and autofocus
  }

  function handleSearchClose() {
    showSearch = false;
    searchQuery = '';
    searchResults = [];
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleSearchClose();
    }
  }

  async function selectSearchResult(note: Note) {
    // If the note is in a different folder, select that folder first
    if (note.folder_id) {
      const folder = foldersStore?.folders?.find((f: Folder) => f.id === note.folder_id);
      const selected = foldersStore?.selectedFolder;
      const selectedId = (selected && typeof selected !== 'string') ? selected.id : null;
      
      if (folder && folder.id !== selectedId) {
        handleSelectFolder(folder);
      }
    } else if (foldersStore?.selectedFolder && foldersStore.selectedFolder !== 'uncategorised') {
      handleSelectFolder(null);
    }
    
    // Select the note
    notesStore?.selectNote(note);
    
    // Close search and switch to editor on mobile
    handleSearchClose();
    if (isMobile) {
      activeMobilePane = 'editor';
    }
  }

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
      const serverSince = localStorage.getItem('beck_last_sync');
      const localSince = localStorage.getItem('beck_last_local_sync');

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
      localStorage.setItem('beck_last_sync', new Date().toISOString());
      localStorage.setItem('beck_last_local_sync', new Date().toISOString());
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
      document.addEventListener('mousedown', (e) => {
        const target = e.target as Node;
        if (showEditorMenu && 
            editorMenuButton && !editorMenuButton.contains(target) &&
            (!editorMenuContainer || !editorMenuContainer.contains(target))) {
          showEditorMenu = false;
        }
      });
      
      // Global keyboard shortcut for search (Ctrl+K / Cmd+K)
      document.addEventListener('keydown', (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
          e.preventDefault();
          if (showSearch) {
            handleSearchClose();
          } else {
            handleSearchOpen();
          }
        }
      });
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
        console.log('Initializing app stores...');
        
        // Safety timeout
        const timeout = setTimeout(() => {
          if (!initialized) {
            console.error('Initialization timeout reached');
            initialized = true;
          }
        }, 5000);
        
        settingsStore.loadSettings();

        // Auto-sync wiring (Tauri only)
        if (isTauri) {
          const wrapMutation = (obj: any, key: string) => {
            const original = obj?.[key];
            if (typeof original !== 'function') return;
            obj[key] = async (...args: any[]) => {
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

          window.addEventListener('beck:local-change', handleLocalChange);
        } else {
          if (autoPullInterval) clearInterval(autoPullInterval);
          autoPullInterval = setInterval(() => {
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
        
        console.log('Loading notes and folders...');
        await Promise.all([
          notesStore.loadNotes(),
          foldersStore.loadFolders()
        ]);
        
        clearTimeout(timeout);
        initialized = true;
        console.log('App initialized successfully');
      } catch (error) {
        console.error('Error during initialization:', error);
        initError = error instanceof Error ? error.message : 'Failed to initialize';
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
        window.removeEventListener('beck:local-change', handleLocalChange);
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
    
    // Determine parent folder: priority to selectedFolder, fallback to lastSelectedFolderId
    let folderId: string | undefined = undefined;
    const selected = foldersStore.selectedFolder;
    
    if (selected && typeof selected !== 'string') {
      folderId = selected.id;
    } else if (lastSelectedFolderId) {
      folderId = lastSelectedFolderId;
    }
    
    // If we explicitly have no selection (cleared by clicking background), ensure folderId is undefined
    if (selectedFolderIds.size === 0 && !selected) {
      folderId = undefined;
    }
    
    const note = await notesStore.createNote(folderId);
    if (note) {
      if (folderId) {
        const next = new Set(expandedFolders);
        next.add(folderId);
        expandedFolders = next;
        console.log('Expanding parent folder after note creation:', folderId);
      }
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

  // Auto-select folder name when EDITING an existing folder
  $effect(() => {
    if (editingFolderId && !justCreatedFolder && folderInput && settingsStore?.autoSelectFolderNameOnEdit) {
      setTimeout(() => {
        folderInput?.focus();
        folderInput?.select();
      }, 0);
    }
  });

  function handleTitleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && editorElement) {
      e.preventDefault();
      editorElement?.focus?.();
    }
  }

  async function handleCreateFolder() {
    console.log('handleCreateFolder called');
    if (!foldersStore) {
      alert('Folders store not initialized');
      return;
    }
    
    // Determine parent folder: priority to selectedFolder, fallback to lastSelectedFolderId
    let parentId: string | null = null;
    const selected = foldersStore.selectedFolder;
    
    if (selected && typeof selected !== 'string') {
      parentId = selected.id;
    } else if (lastSelectedFolderId) {
      parentId = lastSelectedFolderId;
    }

    // If we explicitly have no selection, ensure parentId is null for top-level
    if (selectedFolderIds.size === 0 && !selected) {
      parentId = null;
    }
    
    console.log('Creating folder with parentId:', parentId);
    const folder = await foldersStore.createFolder(parentId);
    if (folder) {
      if (parentId) {
        const next = new Set(expandedFolders);
        next.add(parentId);
        expandedFolders = next;
        console.log('Expanding parent folder after folder creation:', parentId);
      }
      editingFolderId = folder.id;
      editingFolderName = folder.name;
      justCreatedFolder = true;
    } else {
      alert('Failed to create folder - see console');
    }
  }

  function handleSidebarBackgroundClick(e: MouseEvent) {
    // Only trigger if clicking the actual background, not a child
    if (e.target === e.currentTarget) {
      console.log('Sidebar background clicked - clearing selections');
      selectedFolderIds = new Set();
      lastSelectedFolderId = null;
      selectedNoteIds = new Set();
      lastSelectedNoteId = null;
      foldersStore.selectedFolder = null;
      foldersStore.selectFolder(null);
      notesStore.loadNotes();
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

  function flattenFolderTree(nodes: FolderNode[], expanded: Set<string>, depth = 0, acc: FolderListItem[] = []) {
    for (const node of nodes) {
      acc.push({ folder: node, depth });
      if (node.children.length && expanded.has(node.id)) {
        flattenFolderTree(node.children, expanded, depth + 1, acc);
      }
    }
    return acc;
  }

  const flatFolders = $derived.by(() => {
    const folders = foldersStore?.folders || [];
    const expanded = expandedFolders;
    return flattenFolderTree(buildFolderTree(folders), expanded);
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
    
    // If the clicked folder is part of a selection, delete the entire selection
    if (selectedFolderIds.size > 0 && selectedFolderIds.has(folderId)) {
        handleDeleteFolders();
        return;
    }

    folderToDelete = folderId;
    if (settingsStore?.confirmFolderDelete) {
        showDeleteFolderConfirm = true;
    } else {
        confirmDeleteFolder();
    }
  }

  async function confirmDeleteFolder() {
    if (!foldersStore || !notesStore) return;
    
    if (dontAskAgain && settingsStore) {
      settingsStore.confirmFolderDelete = false;
    }
    
    // Determine the next folder to select after deletion
    let nextFolderToSelect: Folder | null = null;
    
    if (selectedFolderIds.size > 0) {
        const sortedIds = flatFolders.filter(f => selectedFolderIds.has(f.folder.id)).map(f => f.folder.id);
        const lastDeletedId = sortedIds[sortedIds.length - 1];
        const lastIdx = flatFolders.findIndex(f => f.folder.id === lastDeletedId);
        
        if (lastIdx < flatFolders.length - 1) {
            nextFolderToSelect = flatFolders[lastIdx + 1].folder;
        } else if (lastIdx > 0) {
            // Find first preceding non-deleted folder
            for (let i = lastIdx - 1; i >= 0; i--) {
                if (!selectedFolderIds.has(flatFolders[i].folder.id)) {
                    nextFolderToSelect = flatFolders[i].folder;
                    break;
                }
            }
        }

        for (const id of selectedFolderIds) {
            await foldersStore.deleteFolder(id);
        }
        selectedFolderIds = new Set();
    } else if (folderToDelete) {
        const idx = flatFolders.findIndex(f => f.folder.id === folderToDelete);
        if (idx < flatFolders.length - 1) {
            nextFolderToSelect = flatFolders[idx + 1].folder;
        } else if (idx > 0) {
            nextFolderToSelect = flatFolders[idx - 1].folder;
        }
        
        await foldersStore.deleteFolder(folderToDelete);
    }
    
    // Reload and update selection
    await foldersStore.loadFolders();
    await notesStore.loadNotes();
    
    if (nextFolderToSelect) {
        handleSelectFolder(nextFolderToSelect);
    } else {
        handleSelectFolder(null); // Fallback to "All Notes"
    }
    
    showDeleteFolderConfirm = false;
    folderToDelete = null;
    dontAskAgain = false;
  }

  function cancelDeleteFolder() {
    showDeleteFolderConfirm = false;
    folderToDelete = null;
    dontAskAgain = false;
  }

  function handleSelectFolder(folder: Folder | null | 'uncategorised', event?: MouseEvent) {
    console.log('handleSelectFolder called:', { folder, event, ctrlKey: event?.ctrlKey, shiftKey: event?.shiftKey, metaKey: event?.metaKey });
    if (!foldersStore || !notesStore) return;

    // Deselect notes when a folder is clicked
    selectedNoteIds = new Set();
    lastSelectedNoteId = null;
    if (!event?.ctrlKey && !event?.metaKey && !event?.shiftKey) {
        notesStore.selectNote(null);
    }

    // Handle "All Notes" (null) click - clear selection
    if (folder === null) {
      console.log('Clearing selection - All Notes clicked');
      selectedFolderIds = new Set();
      lastSelectedFolderId = null;
      selectedNoteIds = new Set();
      lastSelectedNoteId = null;
      foldersStore.selectFolder(null);
      notesStore.selectNote(null);
      notesStore.loadNotes();
      if (isMobile) activeMobilePane = 'notes';
      return;
    }

    // Handle "Uncategorised" click - clear selection
    if (folder === 'uncategorised') {
      selectedFolderIds = new Set();
      lastSelectedFolderId = null;
      selectedNoteIds = new Set();
      lastSelectedNoteId = null;
      foldersStore.selectedFolder = 'uncategorised';
      notesStore.selectNote(null);
      notesStore.loadNotes(undefined, true);
      if (isMobile) activeMobilePane = 'notes';
      return;
    }

    // Handle regular folder clicks
    const id = folder.id;
    console.log('Regular folder click, id:', id, 'current selectedFolderIds:', [...selectedFolderIds]);

    // Toggle Selection (Ctrl/Cmd+Click)
    if (event?.ctrlKey || event?.metaKey) {
      console.log('Ctrl/Cmd+Click detected!');
      const newSet = new Set(selectedFolderIds);
      if (newSet.has(id)) {
        newSet.delete(id);
        console.log('Removing from selection');
      } else {
        newSet.add(id);
        console.log('Adding to selection');
      }
      selectedFolderIds = newSet;
      console.log('New selectedFolderIds:', [...newSet]);
      lastSelectedFolderId = id;
      
      // In single sidebar mode, just return after multi-select
      if (settingsStore?.singleSidebarMode) {
        return;
      }
      // In regular mode, also select the folder for the notes panel if it's the only one
      if (newSet.size === 1) {
        const selectedId = [...newSet][0];
        const selectedFolder = foldersStore.folders.find((f: Folder) => f.id === selectedId);
        if (selectedFolder) {
          foldersStore.selectFolder(selectedFolder);
          notesStore.selectNote(null);
          notesStore.loadNotes(selectedFolder.id);
        }
      }
      return;
    }

    // Range Selection (Shift+Click)
    if (event?.shiftKey && flatFolders.length > 0) {
      // If no previous selection, treat as first selection
      const anchorId = lastSelectedFolderId || id;
      const clickedIndex = flatFolders.findIndex(f => f.folder.id === id);
      const lastIndex = flatFolders.findIndex(f => f.folder.id === anchorId);
      
      if (clickedIndex !== -1 && lastIndex !== -1) {
        const start = Math.min(clickedIndex, lastIndex);
        const end = Math.max(clickedIndex, lastIndex);
        
        // Start fresh selection for the range
        const newSet = new Set<string>();
        for (let i = start; i <= end; i++) {
          newSet.add(flatFolders[i].folder.id);
        }
        selectedFolderIds = newSet;
        // Don't update lastSelectedFolderId to keep the anchor for subsequent shift-clicks
        
        // In single sidebar mode, just return after range select
        if (settingsStore?.singleSidebarMode) {
          return;
        }
        // In regular mode, select the clicked folder for the notes panel
        foldersStore.selectFolder(folder);
        notesStore.selectNote(null);
        notesStore.loadNotes(folder.id);
        return;
      }
    }

    // Standard click behavior
    selectedFolderIds = new Set([id]);
    lastSelectedFolderId = id;

    if (settingsStore?.singleSidebarMode) {
      return; 
    }

    // Regular two-panel mode
    foldersStore.selectFolder(folder);
    notesStore.selectNote(null);
    notesStore.loadNotes(folder.id);
    if (isMobile) activeMobilePane = 'notes';
  }

  function handleSelectNote(note: Note, event?: MouseEvent) {
    if (!notesStore) return;
    
    console.log('handleSelectNote called:', { note: note.id, ctrlKey: event?.ctrlKey, shiftKey: event?.shiftKey, metaKey: event?.metaKey });

    // Clear folder selection when selecting notes
    selectedFolderIds = new Set();
    
    const id = note.id;
    const visibleNotes = notesStore.notes.filter((n: Note) => !n.is_deleted);

    // Toggle Selection (Ctrl/Cmd+Click)
    if (event?.ctrlKey || event?.metaKey) {
      console.log('Ctrl/Cmd+Click on note detected!');
      const newSet = new Set(selectedNoteIds);
      if (newSet.has(id)) {
        newSet.delete(id);
        console.log('Removing note from selection');
      } else {
        newSet.add(id);
        console.log('Adding note to selection');
      }
      selectedNoteIds = newSet;
      console.log('New selectedNoteIds:', [...newSet]);
      lastSelectedNoteId = id;
      
      // If only one note selected, also select it for editing
      if (newSet.size === 1) {
        const selectedId = [...newSet][0];
        const selectedNote = notesStore.notes.find((n: Note) => n.id === selectedId);
        if (selectedNote) {
          notesStore.selectNote(selectedNote);
        }
      }
      return;
    }

    // Range Selection (Shift+Click)
    if (event?.shiftKey && visibleNotes.length > 0) {
      console.log('Shift+Click on note detected!');
      const anchorId = lastSelectedNoteId || id;
      const clickedIndex = visibleNotes.findIndex((n: Note) => n.id === id);
      const lastIndex = visibleNotes.findIndex((n: Note) => n.id === anchorId);
      
      if (clickedIndex !== -1 && lastIndex !== -1) {
        const start = Math.min(clickedIndex, lastIndex);
        const end = Math.max(clickedIndex, lastIndex);
        
        const newSet = new Set<string>();
        for (let i = start; i <= end; i++) {
          newSet.add(visibleNotes[i].id);
        }
        selectedNoteIds = newSet;
        console.log('Range selected notes:', [...newSet]);
        
        // Also select the clicked note for editing
        notesStore.selectNote(note);
        return;
      }
    }

    // Standard click - clear multi-selection and select single note
    console.log('Standard click on note - selecting single');
    selectedNoteIds = new Set([id]);
    lastSelectedNoteId = id;
    notesStore.selectNote(note);
    if (isMobile) activeMobilePane = 'editor';
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
    
    // If we have a multi-selection, delete that first
    if (selectedNoteIds.size > 0) {
        handleDeleteSelectedNotes();
        return;
    }

    if (notesStore.selectedNote) {
      handleDeleteNoteClick(notesStore.selectedNote);
    }
  }

  function handleDeleteNoteClick(note: Note) {
    // If the clicked note is part of a selection, delete the entire selection
    if (selectedNoteIds.size > 0 && selectedNoteIds.has(note.id)) {
        handleDeleteSelectedNotes();
        return;
    }

    noteToDelete = note;
    if (settingsStore?.confirmNoteDelete) {
      showDeleteConfirm = true;
    } else {
      confirmDeleteNote();
    }
  }

  async function confirmDeleteNote() {
    if (!notesStore) return;
    
    if (dontAskAgain && settingsStore) {
      settingsStore.confirmNoteDelete = false;
    }
    
    // Determine the next note to select after deletion
    let nextNoteToSelect: Note | null = null;
    const visibleNotes = notesStore.notes.filter((n: Note) => !n.is_deleted);
    
    if (selectedNoteIds.size > 0) {
        const sortedIds = visibleNotes.filter(n => selectedNoteIds.has(n.id)).map(n => n.id);
        const lastDeletedId = sortedIds[sortedIds.length - 1];
        const lastIdx = visibleNotes.findIndex(n => n.id === lastDeletedId);
        
        if (lastIdx < visibleNotes.length - 1) {
            nextNoteToSelect = visibleNotes[lastIdx + 1];
        } else if (lastIdx > 0 && !selectedNoteIds.has(visibleNotes[lastIdx - 1].id)) {
            nextNoteToSelect = visibleNotes[lastIdx - 1];
        }

        for (const id of selectedNoteIds) {
            await notesStore.deleteNote(id);
        }
        selectedNoteIds = new Set();
    } else if (noteToDelete) {
        const idx = visibleNotes.findIndex(n => n.id === noteToDelete.id);
        if (idx < visibleNotes.length - 1) {
            nextNoteToSelect = visibleNotes[idx + 1];
        } else if (idx > 0) {
            nextNoteToSelect = visibleNotes[idx - 1];
        }
        
        await notesStore.deleteNote(noteToDelete.id);
    }
    
    // Always clear selection before selecting next
    notesStore.selectNote(null);
    if (nextNoteToSelect) {
        notesStore.selectNote(nextNoteToSelect);
        // Also update the selection sets if needed
        selectedNoteIds = new Set([nextNoteToSelect.id]);
        lastSelectedNoteId = nextNoteToSelect.id;
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

  function handleFolderDragStart(event: DragEvent, folder: Folder) {
    if (settingsStore?.singleSidebarMode && !selectedFolderIds.has(folder.id)) {
        if (event.ctrlKey || event.metaKey) {
             const newSet = new Set(selectedFolderIds);
             newSet.add(folder.id);
             selectedFolderIds = newSet;
             lastSelectedFolderId = folder.id; 
        } else {
            selectedFolderIds = new Set([folder.id]);
            lastSelectedFolderId = folder.id;
        }
    }
    draggedFolder = folder;
    event.dataTransfer!.effectAllowed = 'move';
    event.dataTransfer!.setData('text/plain', folder.id);
  }

  function handleFolderDragEnd() {
    draggedFolder = null;
    dragOverFolder = null;
  }

  function handleFolderDragOver(event: DragEvent, folderId: string | null) {
    // Handle both note and folder dragging
    if (!draggedNote && !draggedFolder) return;
    
    // Prevent dragging a folder into itself or its children
    if (draggedFolder && folderId) {
       if (draggedFolder.id === folderId) return;
       // Check if folderId is a child of draggedFolder (prevent cycles)
       // We can check this by traversing up from folderId in the store
       // implementation detail: for now just basic self-check
    }

    event.stopPropagation();
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
    dragOverFolder = folderId || 'root';
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
    event.stopPropagation();
    event.preventDefault();
    dragOverFolder = null;

    if (!notesStore || !foldersStore) return;

    if (draggedNote) {
        // Check if we're dragging multiple selected notes
        if (selectedNoteIds.size > 0 && selectedNoteIds.has(draggedNote.id)) {
            // Multi-move: move all selected notes
            const promises = [];
            for (const noteId of selectedNoteIds) {
                promises.push(notesStore.moveNote(noteId, folderId));
            }
            await Promise.all(promises);
            selectedNoteIds = new Set();
        } else {
            // Single move
            await notesStore.moveNote(draggedNote.id, folderId);
        }
        
        // Reload notes for current view if needed
        const selected = foldersStore.selectedFolder;
        if (selected === 'uncategorised') {
          await notesStore.loadNotes(undefined, true);
        } else if (selected && typeof selected !== 'string') {
          await notesStore.loadNotes(selected.id);
        } else {
          await notesStore.loadNotes();
        }
        draggedNote = null;
    } else if (draggedFolder) {
        // Update the folder's parent
        if (folderId === draggedFolder.id) return; // Can't drop on self

        if (selectedFolderIds.size > 0 && selectedFolderIds.has(draggedFolder.id)) {
             // Multi move
             const promises = [];
             for(const id of selectedFolderIds) {
                 if (id === folderId) continue;
                 const draggedItem = flatFolders.find(f => f.folder.id === id);
                 if (draggedItem) {
                      promises.push(foldersStore.updateFolder({
                         ...draggedItem.folder,
                         parent_id: folderId
                      }));
                 }
             }
             await Promise.all(promises);
             selectedFolderIds.clear();
        } else {
             // Single move
            await foldersStore.updateFolder({
                ...draggedFolder,
                parent_id: folderId
            });
        }
        
        draggedFolder = null;
    }
  }

  async function handleExportPdf() {
    if (!notesStore?.selectedNote) return;
    showEditorMenu = false;
    exportStatus = 'Initializing export...';
    
    const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;

    if (isTauri) {
      try {
        const { save } = await import('@tauri-apps/plugin-dialog');
        const { writeFile } = await import('@tauri-apps/plugin-fs');
        const { jsPDF } = await import('jspdf');

        const lastSaveDir = localStorage.getItem('beck_last_save_dir') || '';
        const fileName = `${notesStore.selectedNote.title || 'Untitled'}.pdf`;
        
        let defaultPath = fileName;
        if (lastSaveDir) {
          const sep = lastSaveDir.includes('\\') ? '\\' : '/';
          defaultPath = `${lastSaveDir}${sep}${fileName}`;
        }

        const path = await save({
          filters: [{ name: 'PDF', extensions: ['pdf'] }],
          defaultPath
        });

        if (!path) {
          exportStatus = '';
          return;
        }

        const lastSlash = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
        if (lastSlash !== -1) {
          localStorage.setItem('beck_last_save_dir', path.substring(0, lastSlash));
        }

        exportStatus = 'Capturing note content...';
        
        const title = notesStore.selectedNote.title || 'Untitled';
        const editorDom = document.querySelector('.collaborative-editor .tiptap');
        const htmlContent = (notesStore.selectedNote.content || '').trim() || (editorDom?.innerHTML || '');
        
        if (!htmlContent.trim()) {
          throw new Error('Note is empty or failed to load content for export.');
        }

        exportStatus = 'Rendering PDF...';

        // Create PDF with text-based rendering (more reliable in Tauri)
        const doc = new jsPDF({
          orientation: 'portrait',
          unit: 'mm',
          format: 'a4'
        });

        const pageWidth = 210;
        const pageHeight = 297;
        const margin = 20;
        const contentWidth = pageWidth - (margin * 2);
        let yPos = margin;

        // Helper to add new page if needed
        const checkPageBreak = (height: number) => {
          if (yPos + height > pageHeight - margin) {
            doc.addPage();
            yPos = margin;
          }
        };

        // Add title
        doc.setFont('helvetica', 'bold');
        doc.setFontSize(24);
        const titleLines = doc.splitTextToSize(title, contentWidth);
        checkPageBreak(titleLines.length * 10);
        doc.text(titleLines, margin, yPos);
        yPos += titleLines.length * 10 + 5;

        // Add separator line
        doc.setDrawColor(0);
        doc.setLineWidth(0.5);
        doc.line(margin, yPos, pageWidth - margin, yPos);
        yPos += 10;

        // Parse HTML and render as formatted text
        const tempDiv = document.createElement('div');
        tempDiv.innerHTML = htmlContent;

        // Helper to convert image to base64
        const imageToBase64 = async (imgSrc: string): Promise<{data: string, format: string} | null> => {
          try {
            // Handle Tauri asset:// protocol by reading from the actual DOM if needed
            if (imgSrc.startsWith('asset://') || imgSrc.startsWith('tauri://') || imgSrc.startsWith('file://')) {
              // Try to find the original image in the editor DOM and use canvas to get base64
              const editorImg = editorDom?.querySelector(`img[src="${imgSrc}"]`) as HTMLImageElement;
              if (editorImg && editorImg.complete && editorImg.naturalWidth > 0) {
                const canvas = document.createElement('canvas');
                canvas.width = editorImg.naturalWidth;
                canvas.height = editorImg.naturalHeight;
                const ctx = canvas.getContext('2d');
                if (ctx) {
                  ctx.drawImage(editorImg, 0, 0);
                  const base64 = canvas.toDataURL('image/png');
                  return { data: base64, format: 'PNG' };
                }
              }
              return null;
            }
            
            // Handle http/https and data URLs
            if (imgSrc.startsWith('data:')) {
              const match = imgSrc.match(/^data:image\/(png|jpeg|jpg|gif|webp);base64,/);
              const format = match ? (match[1] === 'jpg' ? 'JPEG' : match[1].toUpperCase()) : 'PNG';
              return { data: imgSrc, format };
            }
            
            const response = await fetch(imgSrc);
            const blob = await response.blob();
            const base64 = await new Promise<string>((resolve) => {
              const reader = new FileReader();
              reader.onloadend = () => resolve(reader.result as string);
              reader.readAsDataURL(blob);
            });
            // Extract format from data URL (e.g., "data:image/png;base64,...")
            const match = base64.match(/^data:image\/(png|jpeg|jpg|gif|webp);base64,/);
            const format = match ? (match[1] === 'jpg' ? 'JPEG' : match[1].toUpperCase()) : 'PNG';
            return { data: base64, format };
          } catch (e) {
            console.warn('Failed to load image:', imgSrc, e);
            return null;
          }
        };

        // Also try to get images directly from the live DOM using canvas
        const getImageFromDOM = (src: string): {data: string, format: string, width: number, height: number} | null => {
          try {
            const liveImg = editorDom?.querySelector(`img[src="${src}"]`) as HTMLImageElement;
            if (!liveImg) {
              // Try finding by partial src match
              const allImgs = editorDom?.querySelectorAll('img') || [];
              for (const img of Array.from(allImgs)) {
                if ((img as HTMLImageElement).src === src || src.includes((img as HTMLImageElement).src) || (img as HTMLImageElement).src.includes(src)) {
                  if ((img as HTMLImageElement).complete && (img as HTMLImageElement).naturalWidth > 0) {
                    const canvas = document.createElement('canvas');
                    canvas.width = (img as HTMLImageElement).naturalWidth;
                    canvas.height = (img as HTMLImageElement).naturalHeight;
                    const ctx = canvas.getContext('2d');
                    if (ctx) {
                      ctx.drawImage(img as HTMLImageElement, 0, 0);
                      return { 
                        data: canvas.toDataURL('image/png'), 
                        format: 'PNG',
                        width: (img as HTMLImageElement).naturalWidth,
                        height: (img as HTMLImageElement).naturalHeight
                      };
                    }
                  }
                }
              }
              return null;
            }
            if (liveImg.complete && liveImg.naturalWidth > 0) {
              const canvas = document.createElement('canvas');
              canvas.width = liveImg.naturalWidth;
              canvas.height = liveImg.naturalHeight;
              const ctx = canvas.getContext('2d');
              if (ctx) {
                ctx.drawImage(liveImg, 0, 0);
                return { 
                  data: canvas.toDataURL('image/png'), 
                  format: 'PNG',
                  width: liveImg.naturalWidth,
                  height: liveImg.naturalHeight
                };
              }
            }
          } catch (e) {
            console.warn('Failed to get image from DOM:', e);
          }
          return null;
        };

        // Collect all images - use src as key for matching
        const imageDataMap = new Map<string, {data: string, format: string, width: number, height: number}>();
        const imgElements = tempDiv.querySelectorAll('img');
        
        for (const img of Array.from(imgElements)) {
          const src = img.getAttribute('src') || img.src;
          if (src && !imageDataMap.has(src)) {
            // First try getting from live DOM (works for asset://, tauri://, etc.)
            const domImage = getImageFromDOM(src);
            if (domImage) {
              imageDataMap.set(src, domImage);
            } else {
              // Fallback to fetch
              const fetchedImage = await imageToBase64(src);
              if (fetchedImage) {
                imageDataMap.set(src, { ...fetchedImage, width: 200, height: 150 });
              }
            }
          }
        }

        const processNode = async (node: Node): Promise<void> => {
          if (node.nodeType === Node.TEXT_NODE) {
            const text = node.textContent?.trim();
            if (text) {
              doc.setFont('times', 'normal');
              doc.setFontSize(12);
              const lines = doc.splitTextToSize(text, contentWidth);
              checkPageBreak(lines.length * 5);
              doc.text(lines, margin, yPos);
              yPos += lines.length * 5 + 2;
            }
          } else if (node.nodeType === Node.ELEMENT_NODE) {
            const el = node as HTMLElement;
            const tagName = el.tagName.toLowerCase();

            // Check for KaTeX elements - render as formatted math text
            if (el.classList.contains('katex') || el.classList.contains('math-node') || el.closest('.katex')) {
              // Skip if this is a child of katex (already handled by parent)
              if (el.closest('.katex') && !el.classList.contains('katex')) {
                return;
              }
              // Try to get the LaTeX source from the annotation element
              const annotation = el.querySelector('annotation[encoding="application/x-tex"]');
              let mathText = annotation?.textContent || '';
              if (!mathText) {
                // Fallback: get visible text from .katex-html
                const katexHtml = el.querySelector('.katex-html');
                mathText = katexHtml?.textContent || el.textContent || '';
              }
              if (mathText.trim()) {
                doc.setFont('courier', 'italic');
                doc.setFontSize(11);
                doc.setTextColor(80, 80, 80);
                const mathLines = doc.splitTextToSize(`[Math: ${mathText.trim()}]`, contentWidth);
                checkPageBreak(mathLines.length * 5);
                doc.text(mathLines, margin, yPos);
                yPos += mathLines.length * 5 + 3;
                doc.setTextColor(0, 0, 0);
              }
              return;
            }

            switch (tagName) {
              case 'h1':
                doc.setFont('helvetica', 'bold');
                doc.setFontSize(20);
                checkPageBreak(10);
                yPos += 5;
                const h1Lines = doc.splitTextToSize(el.textContent || '', contentWidth);
                doc.text(h1Lines, margin, yPos);
                yPos += h1Lines.length * 8 + 5;
                break;
              case 'h2':
                doc.setFont('helvetica', 'bold');
                doc.setFontSize(16);
                checkPageBreak(8);
                yPos += 4;
                const h2Lines = doc.splitTextToSize(el.textContent || '', contentWidth);
                doc.text(h2Lines, margin, yPos);
                yPos += h2Lines.length * 6 + 4;
                break;
              case 'h3':
                doc.setFont('helvetica', 'bold');
                doc.setFontSize(14);
                checkPageBreak(7);
                yPos += 3;
                const h3Lines = doc.splitTextToSize(el.textContent || '', contentWidth);
                doc.text(h3Lines, margin, yPos);
                yPos += h3Lines.length * 5.5 + 3;
                break;
              case 'p':
                // Process paragraph children to handle inline elements properly
                for (const child of Array.from(el.childNodes)) {
                  await processNode(child);
                }
                yPos += 2; // Add paragraph spacing
                break;
              case 'ul':
              case 'ol':
                doc.setFont('times', 'normal');
                doc.setFontSize(12);
                let itemNum = 1;
                for (const li of Array.from(el.querySelectorAll(':scope > li'))) {
                  const bullet = tagName === 'ul' ? '•' : `${itemNum}.`;
                  const liText = li.textContent?.trim() || '';
                  const liLines = doc.splitTextToSize(liText, contentWidth - 10);
                  checkPageBreak(liLines.length * 5);
                  doc.text(bullet, margin, yPos);
                  doc.text(liLines, margin + 8, yPos);
                  yPos += liLines.length * 5 + 2;
                  itemNum++;
                }
                yPos += 3;
                break;
              case 'blockquote':
                doc.setFont('times', 'italic');
                doc.setFontSize(11);
                doc.setDrawColor(150);
                doc.setLineWidth(0.3);
                const bqText = el.textContent?.trim() || '';
                const bqLines = doc.splitTextToSize(bqText, contentWidth - 15);
                checkPageBreak(bqLines.length * 5 + 4);
                doc.line(margin, yPos - 2, margin, yPos + bqLines.length * 5);
                doc.text(bqLines, margin + 5, yPos);
                yPos += bqLines.length * 5 + 5;
                doc.setDrawColor(0);
                break;
              case 'pre':
              case 'code':
                doc.setFont('courier', 'normal');
                doc.setFontSize(10);
                doc.setFillColor(245, 245, 245);
                const codeText = el.textContent?.trim() || '';
                const codeLines = doc.splitTextToSize(codeText, contentWidth - 10);
                checkPageBreak(codeLines.length * 4 + 8);
                doc.rect(margin, yPos - 3, contentWidth, codeLines.length * 4 + 6, 'F');
                doc.text(codeLines, margin + 3, yPos);
                yPos += codeLines.length * 4 + 8;
                break;
              case 'strong':
              case 'b':
                doc.setFont('times', 'bold');
                doc.setFontSize(12);
                const boldText = el.textContent?.trim();
                if (boldText) {
                  const boldLines = doc.splitTextToSize(boldText, contentWidth);
                  doc.text(boldLines, margin, yPos);
                  yPos += boldLines.length * 5 + 2;
                }
                break;
              case 'em':
              case 'i':
                doc.setFont('times', 'italic');
                doc.setFontSize(12);
                const italicText = el.textContent?.trim();
                if (italicText) {
                  const italicLines = doc.splitTextToSize(italicText, contentWidth);
                  doc.text(italicLines, margin, yPos);
                  yPos += italicLines.length * 5 + 2;
                }
                break;
              case 'hr':
                checkPageBreak(10);
                yPos += 3;
                doc.setDrawColor(180);
                doc.setLineWidth(0.3);
                doc.line(margin, yPos, pageWidth - margin, yPos);
                yPos += 7;
                doc.setDrawColor(0);
                break;
              case 'br':
                yPos += 4;
                break;
              case 'img':
                const imgSrc = (el as HTMLImageElement).getAttribute('src') || (el as HTMLImageElement).src;
                const imgData = imageDataMap.get(imgSrc);
                if (imgData) {
                  try {
                    // Calculate image dimensions (max width = contentWidth, maintain aspect ratio)
                    const naturalWidth = imgData.width || 200;
                    const naturalHeight = imgData.height || 150;
                    const aspectRatio = naturalHeight / naturalWidth;
                    
                    let imgWidth = Math.min(contentWidth, 100); // Max 100mm wide
                    let imgHeight = imgWidth * aspectRatio;
                    
                    // Limit height to avoid huge images
                    if (imgHeight > 100) {
                      imgHeight = 100;
                      imgWidth = imgHeight / aspectRatio;
                    }
                    
                    checkPageBreak(imgHeight + 5);
                    doc.addImage(imgData.data, imgData.format, margin, yPos, imgWidth, imgHeight);
                    yPos += imgHeight + 5;
                  } catch (imgErr) {
                    console.warn('Failed to add image to PDF:', imgErr);
                    doc.setFont('times', 'italic');
                    doc.setFontSize(10);
                    doc.setTextColor(150);
                    doc.text('[Image could not be rendered]', margin, yPos);
                    doc.setTextColor(0);
                    yPos += 5;
                  }
                } else {
                  // Image not found - show placeholder
                  doc.setFont('times', 'italic');
                  doc.setFontSize(10);
                  doc.setTextColor(150);
                  doc.text('[Image]', margin, yPos);
                  doc.setTextColor(0);
                  yPos += 5;
                }
                break;
              case 'span':
                // Check for special span types
                if (el.classList.contains('katex') || el.closest('.katex')) {
                  // Already handled above
                  return;
                }
                // Process children for regular spans
                for (const child of Array.from(el.childNodes)) {
                  await processNode(child);
                }
                break;
              default:
                // Process children for container elements like div
                for (const child of Array.from(el.childNodes)) {
                  await processNode(child);
                }
                break;
            }
          }
        };

        for (const node of Array.from(tempDiv.childNodes)) {
          await processNode(node);
        }

        // Output as blob
        const pdfBlob = doc.output('blob');
        const uint8 = new Uint8Array(await pdfBlob.arrayBuffer());
        await writeFile(path, uint8);
        
        exportedPath = path;
        showExportSuccess = true;
        exportStatus = '';
      } catch (err) {
        console.error('Failed to export PDF:', err);
        exportStatus = '';
        alert('Failed to export PDF: ' + (err instanceof Error ? err.message : String(err)));
      }
    } else {
      window.print();
    }
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
        <p class="text-gray-600">Loading Beck...</p>
        <p class="text-gray-400 text-sm mt-2">This should only take a moment</p>
      {/if}
    </div>
  </div>
{:else}
<div class="flex h-full overflow-hidden bg-gray-50 flex-col md:flex-row" class:cursor-col-resize={isDraggingLeft || isDraggingRight}>
  <!-- Left Sidebar (Folders) -->
  <aside 
    bind:this={leftSidebarElement} 
    class="border-r border-gray-200 bg-white flex flex-col {isDraggingLeft ? '' : 'transition-all'} {leftSidebarCollapsed ? 'w-0 -ml-px' : ''} overflow-hidden {dragOverFolder === 'root' ? 'bg-indigo-50/50 ring-2 ring-indigo-400 ring-inset' : ''}" 
    style="width: {isMobile ? (activeMobilePane === 'folders' ? '100%' : '0px') : (leftSidebarCollapsed ? '0px' : leftSidebarWidth + 'px')}"
    class:hidden={isMobile && activeMobilePane !== 'folders'}
    onclick={handleSidebarBackgroundClick}
    ondragover={(e) => (settingsStore?.singleSidebarMode || draggedNote || draggedFolder) && handleFolderDragOver(e, null)}
    ondragleave={(e) => handleFolderDragLeave(e)}
    ondrop={(e) => (settingsStore?.singleSidebarMode || draggedFolder || draggedNote) && handleFolderDrop(e, null)}
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
            onclick={handleSearchOpen}
            title="Search notes (Ctrl+K)"
          >
            <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
          </button>
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
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={leftSidebarCollapsed ? "M13 5l7 7-7 7M5 5l7 7-7 7" : "M11 19l-7-7 7-7M19 19l-7-7 7-7"} />
            </svg>
          </button>
        {/if}
      </div>
    </div>
   </div>

    <!-- Folders Section -->
    <div 
      class="flex-1 overflow-y-auto"
      onclick={handleSidebarBackgroundClick}
      ondragover={(e) => settingsStore?.singleSidebarMode && handleFolderDragOver(e, null)}
      ondragleave={(e) => settingsStore?.singleSidebarMode && handleFolderDragLeave(e)}
      ondrop={(e) => settingsStore?.singleSidebarMode && handleFolderDrop(e, null)}
      role="list"
    >
      <div class="px-4 py-2 space-y-1">
        {#if settingsStore?.showAllNotesFolder && !settingsStore?.singleSidebarMode}
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
        
        {#if settingsStore?.showUncategorisedFolder && !settingsStore?.singleSidebarMode}
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
      {:else}
        {#if flatFolders.length === 0 && (!settingsStore?.singleSidebarMode || notesStore?.notes?.filter((n: Note) => !n.folder_id && !n.is_deleted).length === 0)}
          <div class="px-6 py-8 text-center">
            <svg class="w-12 h-12 mx-auto text-gray-300 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
            </svg>
            <p class="text-gray-500 text-sm">No folders yet</p>
            <button class="text-indigo-600 text-xs font-medium mt-2 hover:underline" onclick={handleCreateFolder}>
              Create your first folder
            </button>
          </div>
        {:else}
          <div class="px-4 pb-4 space-y-1 transition-opacity duration-200" class:opacity-50={foldersStore.loading}>
            {#if settingsStore?.singleSidebarMode}
                {#each notesStore.notes.filter((n: Note) => !n.folder_id && !n.is_deleted) as note}
                    <div class="group relative">
                        <div
                            class="w-full text-left py-2 rounded-lg hover:bg-gray-100 transition-colors flex items-center justify-between cursor-pointer {selectedNoteIds.has(note.id) ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'}"
                            style="padding-left: 12px; padding-right: 12px;"
                            onclick={(e) => handleSelectNote(note, e)}
                            draggable="true"
                            ondragstart={(e) => handleNoteDragStart(e, note)}
                            ondragend={handleNoteDragEnd}
                            role="button"
                            tabindex="0"
                        >
                            <span class="flex items-center gap-2 min-w-0 pointer-events-none">
                                <svg class="w-4 h-4 text-gray-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
                                </svg>
                                <span class="truncate text-sm">{note.title || 'Untitled Note'}</span>
                            </span>
                            
                            <div class="flex gap-1 pointer-events-auto transition-opacity" class:opacity-0={!isMobile} class:group-hover:opacity-100={!isMobile}>
                                <div
                                  class="p-1 hover:bg-red-100 rounded text-red-600 cursor-pointer"
                                  onclick={(e) => { e.stopPropagation(); handleDeleteNoteClick(note); }}
                                  role="button"
                                  tabindex="0"
                                  title="Delete Note"
                                >
                                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                                  </svg>
                                </div>
                            </div>
                        </div>
                    </div>
                {/each}
            {/if}

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
                    onfocus={(e) => settingsStore?.autoSelectFolderNameOnEdit && e.currentTarget.select()}
                    autofocus
                  />
                </div>
              {:else}
                <div
                  class="w-full text-left py-2 rounded-lg hover:bg-gray-100 transition-colors flex items-center justify-between cursor-pointer {selectedFolderIds.has(folder.id) ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'} {dragOverFolder === folder.id ? 'ring-2 ring-indigo-400 bg-indigo-50' : ''}"
                  style={`padding-left: ${12 + item.depth * 16}px; padding-right: 12px;`}
                  onclick={(e) => handleSelectFolder(folder, e)}
                  ondblclick={(e) => { e.stopPropagation(); toggleFolder(folder.id); }}
                  draggable="true"
                  ondragstart={(e) => handleFolderDragStart(e, folder)}
                  ondragend={handleFolderDragEnd}
                  ondragover={(e) => handleFolderDragOver(e, folder.id)}
                  ondragleave={(e) => handleFolderDragLeave(e)}
                  ondrop={(e) => handleFolderDrop(e, folder.id)}
                  ondragenter={(e) => handleFolderDragOver(e, folder.id)}
                  role="button"
                  tabindex="0"
                >
                  <span class="flex items-center pointer-events-none">
                    <button 
                      class="p-1 -ml-1 hover:bg-gray-200 rounded-md transition-all pointer-events-auto"
                      onclick={(e) => { e.stopPropagation(); toggleFolder(folder.id); }}
                      aria-label={expandedFolders.has(folder.id) ? 'Collapse' : 'Expand'}
                    >
                      <svg class="w-3 h-3 text-gray-400 transition-transform {expandedFolders.has(folder.id) ? 'rotate-90' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                      </svg>
                    </button>
                    <svg class="w-4 h-4 mr-2 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
            
            {#if settingsStore?.singleSidebarMode && expandedFolders.has(folder.id)}
                  <div 
                      class="w-full"
                      ondragover={(e) => handleFolderDragOver(e, folder.id)}
                      ondrop={(e) => handleFolderDrop(e, folder.id)}
                      role="group"
                  >
                 {#each notesStore.notes.filter((n: Note) => n.folder_id === folder.id && !n.is_deleted) as note}
                   <div class="group relative">
                        <div
                            class="w-full text-left py-2 rounded-lg hover:bg-gray-100 transition-colors flex items-center justify-between cursor-pointer {selectedNoteIds.has(note.id) ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'}"
                            style={`padding-left: ${12 + (item.depth + 1) * 16}px; padding-right: 12px;`}
                            onclick={(e) => handleSelectNote(note, e)}
                            draggable="true"
                            ondragstart={(e) => handleNoteDragStart(e, note)}
                            ondragend={handleNoteDragEnd}
                            role="button"
                            tabindex="0"
                        >
                            <span class="flex items-center gap-2 min-w-0 pointer-events-none">
                                <svg class="w-4 h-4 text-gray-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
                                </svg>
                                <span class="truncate text-sm">{note.title || 'Untitled Note'}</span>
                            </span>
                            
                            <div class="flex gap-1 pointer-events-auto transition-opacity" class:opacity-0={!isMobile} class:group-hover:opacity-100={!isMobile}>
                                <div
                                  class="p-1 hover:bg-red-100 rounded text-red-600 cursor-pointer"
                                  onclick={(e) => { e.stopPropagation(); handleDeleteNoteClick(note); }}
                                  role="button"
                                  tabindex="0"
                                  title="Delete Note"
                                >
                                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                                  </svg>
                                </div>
                            </div>
                        </div>
                   </div>
                 {/each}
                 </div>
            {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}
    </div>
  </aside>

  <!-- Left Sidebar Expand Button (when collapsed) -->
  {#if leftSidebarCollapsed && !isMobile}
    <button
      class="fixed left-0 top-14 w-6 h-10 bg-white/80 hover:bg-white border-y border-r border-gray-200 rounded-r-full flex items-center justify-center transition-all shadow-sm group z-[45]"
      onclick={() => (leftSidebarCollapsed = false)}
      title="Expand folders sidebar"
    >
      <svg class="w-4 h-4 text-gray-400 group-hover:text-indigo-600 transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M9 5l7 7-7 7" />
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
    class:hidden={(isMobile && activeMobilePane !== 'notes') || settingsStore?.singleSidebarMode}
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
                class="w-full text-left p-4 rounded-lg border border-gray-200 hover:border-indigo-400 hover:bg-indigo-50 transition-all {selectedNoteIds.has(note.id) ? 'bg-indigo-50 border-indigo-400' : 'bg-white'} {draggedNote?.id === note.id ? 'opacity-50' : ''}"
                onclick={(e) => handleSelectNote(note, e)}
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
  {#if rightSidebarCollapsed && !isMobile && !settingsStore?.singleSidebarMode}
    <button
      class="fixed right-0 top-14 w-6 h-10 bg-white/80 hover:bg-white border-y border-l border-gray-200 rounded-l-full flex items-center justify-center transition-all shadow-sm group z-[45]"
      onclick={() => (rightSidebarCollapsed = false)}
      title="Expand notes sidebar"
    >
      <svg class="w-4 h-4 text-gray-400 group-hover:text-indigo-600 transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M15 19l-7-7 7-7" />
      </svg>
    </button>
  {/if}

  <!-- Right Sidebar Resize Handle -->
  {#if !rightSidebarCollapsed && !isMobile && !settingsStore?.singleSidebarMode}
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
        <div class="relative">
          <button
            bind:this={editorMenuButton}
            class="p-2 hover:bg-gray-100 rounded-lg transition-colors text-gray-500"
            onclick={() => (showEditorMenu = !showEditorMenu)}
            title="More options"
          >
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
            </svg>
          </button>
          
          {#if showEditorMenu}
            <div bind:this={editorMenuContainer} class="absolute right-0 mt-2 w-48 bg-white rounded-md shadow-lg py-1 z-50 border border-gray-200">
              <button
                class="w-full text-left px-4 py-2 text-sm text-gray-700 hover:bg-gray-100 flex items-center gap-2"
                onclick={handleExportPdf}
              >
                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
                </svg>
                Export PDF
              </button>
              <button
                class="w-full text-left px-4 py-2 text-sm text-red-600 hover:bg-red-50 flex items-center gap-2"
                onclick={() => { showEditorMenu = false; handleDeleteNote(); }}
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                </svg>
                Delete Note
              </button>
            </div>
          {/if}
        </div>
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

<!-- Search Modal -->
{#if showSearch}
  <div 
    class="fixed inset-0 flex items-start justify-center z-50 pt-[10vh]" 
    style="background-color: rgba(0, 0, 0, 0.4);" 
    onmousedown={(e) => e.target === e.currentTarget && handleSearchClose()}
    onkeydown={handleSearchKeydown}
    role="dialog"
    aria-modal="true"
    aria-label="Search notes"
    tabindex="-1"
    transition:fade={{ duration: 150 }}
  >
    <div class="bg-white rounded-xl shadow-2xl w-full max-w-xl mx-4 overflow-hidden" transition:scale={{ duration: 150, start: 0.95 }}>
      <!-- Search Input -->
      <div class="p-4 border-b border-gray-200">
        <div class="relative">
          <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            bind:this={searchInput}
            bind:value={searchQuery}
            oninput={() => performSearch(searchQuery)}
            type="text"
            placeholder="Search notes..."
            class="w-full pl-10 pr-10 py-3 text-lg border-0 focus:ring-0 focus:outline-none"
            autofocus
          />
          {#if searchQuery}
            <button 
              class="absolute right-3 top-1/2 -translate-y-1/2 p-1 hover:bg-gray-100 rounded"
              onclick={() => { searchQuery = ''; searchResults = []; searchInput?.focus(); }}
              aria-label="Clear search"
            >
              <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          {/if}
        </div>
      </div>
      
      <!-- Search Results -->
      <div class="max-h-[60vh] overflow-y-auto">
        {#if searchQuery && searchResults.length === 0}
          <div class="p-8 text-center text-gray-500">
            <svg class="w-12 h-12 mx-auto text-gray-300 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p class="font-medium">No notes found</p>
            <p class="text-sm mt-1">Try a different search term</p>
          </div>
        {:else if searchResults.length > 0}
          <div class="py-2">
            {#each searchResults as note}
              {@const noteFolder = foldersStore?.folders?.find((f: Folder) => f.id === note.folder_id)}
              <button
                class="w-full text-left px-4 py-3 hover:bg-indigo-50 transition-colors border-b border-gray-100 last:border-b-0"
                onclick={() => selectSearchResult(note)}
              >
                <div class="flex items-start justify-between gap-3">
                  <div class="flex-1 min-w-0">
                    <h4 class="font-medium text-gray-900 truncate">{note.title || 'Untitled Note'}</h4>
                    <p class="text-sm text-gray-500 mt-1 line-clamp-2">
                      {note.content ? stripHtml(note.content).substring(0, 120) : 'No content'}
                    </p>
                  </div>
                  <div class="flex-shrink-0 flex flex-col items-end gap-1">
                    {#if noteFolder}
                      <span class="px-2 py-0.5 bg-indigo-100 text-indigo-700 text-xs rounded flex items-center gap-1">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
                        </svg>
                        {noteFolder.name}
                      </span>
                    {/if}
                    <span class="text-xs text-gray-400">
                      {new Date(note.updated_at).toLocaleDateString()}
                    </span>
                  </div>
                </div>
              </button>
            {/each}
          </div>
        {:else}
          <div class="p-8 text-center text-gray-500">
            <svg class="w-12 h-12 mx-auto text-gray-300 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <p class="font-medium">Search your notes</p>
            <p class="text-sm mt-1">Start typing to find notes by title or content</p>
          </div>
        {/if}
      </div>
      
      <!-- Footer -->
      <div class="px-4 py-2 bg-gray-50 border-t border-gray-200 flex items-center justify-between text-xs text-gray-500">
        <span>
          {#if searchResults.length > 0}
            {searchResults.length} result{searchResults.length === 1 ? '' : 's'}
          {:else}
            Type to search
          {/if}
        </span>
        <span class="flex items-center gap-2">
          <kbd class="px-1.5 py-0.5 bg-gray-200 rounded text-gray-600">Esc</kbd>
          <span>to close</span>
        </span>
      </div>
    </div>
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
            <h3 class="text-lg font-semibold text-gray-900">
              {selectedNoteIds.size > 0 ? `Delete ${selectedNoteIds.size} Notes?` : 'Delete Note?'}
            </h3>
            <p class="text-sm text-gray-500 mt-1">
              {#if selectedNoteIds.size > 0}
                Are you sure you want to delete {selectedNoteIds.size} selected notes? This action cannot be undone.
              {:else}
                Are you sure you want to delete "{noteToDelete?.title || 'Untitled Note'}"? This action cannot be undone.
              {/if}
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
            autofocus
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
            <h3 class="text-lg font-semibold text-gray-900">
              {selectedFolderIds.size > 0 ? `Delete ${selectedFolderIds.size} Folders?` : 'Delete Folder?'}
            </h3>
            <p class="text-sm text-gray-500 mt-1">
              {#if selectedFolderIds.size > 0}
                Are you sure you want to delete {selectedFolderIds.size} selected folders? All notes in these folders will also be deleted.
              {:else}
                Are you sure you want to delete "{foldersStore.folders.find((f: Folder) => f.id === folderToDelete)?.name || 'this folder'}"? All notes in this folder will also be deleted.
              {/if}
            </p>
          </div>
        </div>

        <div class="flex items-center gap-2 mb-6 px-2">
          <input
            type="checkbox"
            id="dont-ask-again-folder"
            bind:checked={dontAskAgain}
            class="w-4 h-4 text-indigo-600 border-gray-300 rounded focus:ring-indigo-500"
          />
          <label for="dont-ask-again-folder" class="text-sm text-gray-700 cursor-pointer">
            Don't ask me again
          </label>
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
            autofocus
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Export Status Overlay -->
{#if exportStatus}
  <div class="fixed inset-0 flex items-center justify-center z-[70] bg-black/20 backdrop-blur-[1px]" transition:fade={{ duration: 150 }}>
    <div class="bg-white rounded-xl shadow-2xl p-8 flex flex-col items-center gap-4 max-w-xs w-full">
      <div class="inline-block w-10 h-10 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin"></div>
      <p class="text-lg font-medium text-gray-900">{exportStatus}</p>
      <p class="text-sm text-gray-500 text-center">This may take a few seconds for long notes...</p>
    </div>
  </div>
{/if}

<!-- PDF Export Success Modal -->
{#if showExportSuccess}
  <div class="fixed inset-0 flex items-center justify-center z-[60]" style="background-color: rgba(0, 0, 0, 0.2);" transition:fade={{ duration: 200 }}>
    <div class="bg-white rounded-lg shadow-2xl max-w-md w-full mx-4 overflow-hidden" transition:scale={{ duration: 200, start: 0.95 }}>
      <div class="bg-emerald-600 p-4 flex justify-center">
        <div class="w-16 h-16 rounded-full bg-white/20 flex items-center justify-center">
          <svg class="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7"/>
          </svg>
        </div>
      </div>
      <div class="p-6 text-center">
        <h3 class="text-xl font-bold text-gray-900 mb-2">Export Successful!</h3>
        <p class="text-gray-600 mb-4">Your note has been saved as a PDF to:</p>
        <div class="bg-gray-50 p-3 rounded border border-gray-200 text-xs font-mono text-gray-500 break-all mb-6">
          {exportedPath}
        </div>
        <button
          class="w-full py-3 bg-emerald-600 text-white rounded-lg font-semibold hover:bg-emerald-700 transition-colors shadow-md"
          onclick={() => (showExportSuccess = false)}
        >
          Great!
        </button>
      </div>
    </div>
  </div>
{/if}