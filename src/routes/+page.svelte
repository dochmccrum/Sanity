<script lang="ts">
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import type { Note } from '$lib/api/notes';
  import type { Folder } from '$lib/api/folders';
  import WysiwygEditor from '$lib/components/WysiwygEditor.svelte';

  let notesStore: any = $state(null);
  let foldersStore: any = $state(null);
  let initialized = $state(false);
  let editingFolderId = $state<string | null>(null);
  let editingFolderName = $state('');
  let initError = $state<string | null>(null);

  onMount(async () => {
    if (browser) {
      try {
        console.log('Initializing stores...');
        
        // Set a timeout to prevent infinite loading
        const timeout = setTimeout(() => {
          console.error('Store initialization timeout');
          initError = 'Failed to load app - timeout';
        }, 5000);
        
        const { createNotesStore } = await import('$lib/stores/notes.svelte');
        const { createFoldersStore } = await import('$lib/stores/folders.svelte');
        notesStore = createNotesStore();
        foldersStore = createFoldersStore();
        
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
          notesStore = createNotesStore();
          foldersStore = createFoldersStore();
        }
        initialized = true;
      }
    }
  });

  async function handleCreateNote() {
    if (!notesStore || !foldersStore) return;
    const note = await notesStore.createNote(foldersStore.selectedFolder?.id);
    if (note) {
      notesStore.selectNote(note);
    }
  }

  async function handleCreateFolder() {
    if (!foldersStore) return;
    const folder = await foldersStore.createFolder(null);
    if (folder) {
      editingFolderId = folder.id;
      editingFolderName = folder.name;
    }
  }

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
    if (confirm('Delete this folder? Notes in it will not be deleted.')) {
      await foldersStore.deleteFolder(folderId);
      if (foldersStore.selectedFolder?.id === folderId) {
        // Reload all notes if we deleted the currently selected folder
        await notesStore.loadNotes();
      }
    }
  }

  function handleSelectFolder(folder: Folder | null) {
    if (!foldersStore || !notesStore) return;
    foldersStore.selectFolder(folder);
    notesStore.selectNote(null);
    // Reload notes filtered by the selected folder
    notesStore.loadNotes(folder?.id);
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

  function handleTitleChange(e: Event) {
    if (!notesStore) return;
    const target = e.target as HTMLInputElement;
    if (notesStore.selectedNote) {
      notesStore.updateNote({
        ...notesStore.selectedNote,
        title: target.value
      });
    }
  }

  async function handleDeleteNote() {
    if (!notesStore || !foldersStore) return;
    if (notesStore.selectedNote && confirm('Delete this note?')) {
      await notesStore.deleteNote(notesStore.selectedNote.id);
      notesStore.selectNote(null);
    }
  }
</script>

{#if !initialized}
  <div class="flex h-screen items-center justify-center bg-gray-50">
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
        <p class="text-gray-600">Loading JFNotes...</p>
        <p class="text-gray-400 text-sm mt-2">This should only take a moment</p>
      {/if}
    </div>
  </div>
{:else}
<div class="flex h-screen overflow-hidden bg-gray-50">
  <!-- Sidebar -->
  <aside class="w-[280px] border-r border-gray-200 bg-white flex flex-col">
    <div class="p-4 border-b border-gray-200">
      <h1 class="text-2xl font-bold text-indigo-600">JFNotes</h1>
      <p class="text-sm text-gray-500 mt-1">Local-first notes</p>
    </div>

    <div class="p-4 space-y-2">
      <button class="btn btn-primary w-full" onclick={handleCreateNote}>
        <svg class="w-5 h-5 inline-block mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>
        </svg>
        New Note
      </button>
      <button class="btn btn-secondary w-full" onclick={handleCreateFolder}>
        <svg class="w-5 h-5 inline-block mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
        </svg>
        New Folder
      </button>
    </div>

    <!-- Folders Section -->
    <div class="flex-1 overflow-y-auto">
      <div class="px-4 py-2">
        <button 
          class="w-full text-left px-3 py-2 rounded-lg hover:bg-gray-100 transition-colors {!foldersStore.selectedFolder ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'}"
          onclick={() => handleSelectFolder(null)}
        >
          <svg class="w-4 h-4 inline-block mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
          </svg>
          All Notes
        </button>
      </div>

      {#if foldersStore.loading}
        <div class="text-center text-gray-500 py-4">
          <div class="inline-block w-6 h-6 border-4 border-indigo-600 border-t-transparent rounded-full animate-spin"></div>
        </div>
      {:else if foldersStore.folders.length > 0}
        <div class="px-4 pb-4 space-y-1">
          {#each foldersStore.folders as folder}
            <div class="group relative">
              {#if editingFolderId === folder.id}
                <div class="flex items-center gap-2">
                  <input
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
                  class="w-full text-left px-3 py-2 rounded-lg hover:bg-gray-100 transition-colors flex items-center justify-between cursor-pointer {foldersStore.selectedFolder?.id === folder.id ? 'bg-indigo-50 text-indigo-600' : 'text-gray-700'}"
                  onclick={() => handleSelectFolder(folder)}
                  role="button"
                  tabindex="0"
                >
                  <span class="flex items-center">
                    <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
                    </svg>
                    <span class="truncate">{folder.name}</span>
                  </span>
                  <div class="opacity-0 group-hover:opacity-100 flex gap-1">
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

  <!-- Notes List -->
  <aside class="w-[320px] border-r border-gray-200 bg-white flex flex-col">
    <div class="p-4 border-b border-gray-200">
      <h2 class="font-semibold text-gray-700">
        {foldersStore.selectedFolder ? foldersStore.selectedFolder.name : 'All Notes'}
      </h2>
    </div>

    <div class="flex-1 overflow-y-auto p-4">
      {#if notesStore.loading}
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
        <div class="space-y-2">
          {#each notesStore.notes as note}
            <button
              class="w-full text-left p-4 rounded-lg border border-gray-200 hover:border-indigo-400 hover:bg-indigo-50 transition-all {notesStore.selectedNote?.id === note.id ? 'bg-indigo-50 border-indigo-400' : 'bg-white'}"
              onclick={() => notesStore.selectNote(note)}
            >
              <h3 class="font-medium text-gray-900 truncate">
                {note.title || 'Untitled Note'}
              </h3>
              <p class="text-sm text-gray-500 mt-1 line-clamp-2">
                {note.content.substring(0, 100) || 'No content'}
              </p>
              <div class="flex items-center justify-between mt-2">
                <span class="text-xs text-gray-400">
                  {new Date(note.updated_at).toLocaleDateString()}
                </span>
                {#if note.is_canvas}
                  <span class="px-2 py-1 bg-purple-100 text-purple-700 text-xs rounded">Canvas</span>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </aside>

  <!-- Editor Area -->
  <main class="flex-1 flex flex-col bg-white">
    {#if notesStore.selectedNote}
      <div class="border-b border-gray-200 p-4 flex items-center justify-between">
        <input
          type="text"
          class="flex-1 text-2xl font-bold bg-transparent border-none outline-none"
          placeholder="Note title..."
          value={notesStore.selectedNote.title}
          oninput={handleTitleChange}
        />
        <div class="flex items-center gap-2">
          <button
            class="btn bg-red-500 text-white hover:bg-red-600"
            onclick={handleDeleteNote}
          >
            Delete
          </button>
        </div>
      </div>

      <div class="flex-1 overflow-hidden">
        <WysiwygEditor
          bind:value={notesStore.selectedNote.content}
          onchange={handleContentChange}
        />
      </div>
    {:else}
      <div class="flex-1 flex items-center justify-center text-gray-400">
        <div class="text-center">
          <svg class="w-24 h-24 mx-auto mb-4 text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
          </svg>
          <p class="text-xl font-medium">No note selected</p>
          <p class="mt-2">Select a note or create a new one</p>
        </div>
      </div>
    {/if}
  </main>
</div>
{/if}
