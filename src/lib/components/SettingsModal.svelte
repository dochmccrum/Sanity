<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  let {
    open = $bindable(false),
    settings,
    isTauri = false,
    onSync
  }: {
    open?: boolean;
    settings: any;
    isTauri?: boolean;
    onSync?: () => Promise<void> | void;
  } = $props();

  let syncPassword = $state('');
  let loginBusy = $state(false);
  let syncBusy = $state(false);
  
  let recordingShortcut = $state<string | null>(null);

  function getShortcutString(e: KeyboardEvent): string {
    const parts = [];
    if (e.ctrlKey || e.metaKey) parts.push('Control');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');
    
    const key = e.key;
    if (!['Control', 'Alt', 'Shift', 'Meta'].includes(key)) {
      parts.push(key);
    }
    
    return parts.join('+');
  }

  function handleShortcutKeyDown(e: KeyboardEvent) {
    if (!recordingShortcut) return;
    e.preventDefault();
    e.stopPropagation();
    
    // Don't record just modifier keys
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return;
    
    const newShortcut = getShortcutString(e);
    settings.updateShortcut(recordingShortcut as any, newShortcut);
    recordingShortcut = null;
  }

  let syncStatus = $state<string | null>(null);

  async function handleLogin() {
    if (!settings?.loginSync) return;
    loginBusy = true;
    syncStatus = null;
    try {
      await settings.loginSync(syncPassword);
      syncStatus = 'Logged in.';
    } catch (e) {
      syncStatus = e instanceof Error ? e.message : 'Login failed';
    } finally {
      loginBusy = false;
    }
  }

  async function handleSyncNow() {
    if (!onSync) return;
    syncBusy = true;
    syncStatus = null;
    try {
      await onSync();
      settings?.refreshLastSync?.();
      syncStatus = 'Synced.';
    } catch (e) {
      syncStatus = e instanceof Error ? e.message : 'Sync failed';
    } finally {
      syncBusy = false;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      open = false;
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 flex items-center justify-center z-50 bg-black/10" onmousedown={handleBackdropClick} transition:fade={{ duration: 200 }}>
    <div class="bg-white shadow-xl w-full h-full flex flex-col md:rounded-lg md:max-w-lg md:mx-4 md:h-auto md:max-h-[90vh]" transition:scale={{ duration: 200, start: 0.95 }}>
      <div class="border-b border-gray-200 p-6 flex justify-between items-center">
        <h2 class="text-2xl font-bold text-gray-900">Settings</h2>
        {#if !isTauri}
          <button class="md:hidden p-2 text-gray-500" onclick={() => open = false}>
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
            </svg>
          </button>
        {/if}
      </div>

      <div class="p-6 space-y-6 overflow-y-auto flex-1">
        <!-- Auto-focus Title Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="auto-focus" class="block font-medium text-gray-900">
              Auto-focus Title
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Automatically focus the title field when creating a new note
            </p>
          </div>
            <button
              id="auto-focus"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.autoFocusTitleOnNewNote
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.autoFocusTitleOnNewNote = !settings.autoFocusTitleOnNewNote;
            }}
            aria-label="Toggle auto-focus title"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.autoFocusTitleOnNewNote
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Auto-highlight Folder Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="auto-highlight-folder" class="block font-medium text-gray-900">
              Auto-highlight Folder Name
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Automatically select the folder name when editing
            </p>
          </div>
            <button
              id="auto-highlight-folder"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.autoSelectFolderNameOnEdit
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.autoSelectFolderNameOnEdit = !settings.autoSelectFolderNameOnEdit;
            }}
            aria-label="Toggle auto-highlight folder name"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.autoSelectFolderNameOnEdit
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Confirm Note Delete Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="confirm-note-delete" class="block font-medium text-gray-900">
              Confirm Note Delete
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Show confirmation dialog before deleting notes
            </p>
          </div>
            <button
              id="confirm-note-delete"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.confirmNoteDelete
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.confirmNoteDelete = !settings.confirmNoteDelete;
            }}
            aria-label="Toggle confirm note delete"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.confirmNoteDelete
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Confirm Folder Delete Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="confirm-folder-delete" class="block font-medium text-gray-900">
              Confirm Folder Delete
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Show confirmation dialog before deleting folders
            </p>
          </div>
            <button
              id="confirm-folder-delete"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.confirmFolderDelete
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.confirmFolderDelete = !settings.confirmFolderDelete;
            }}
            aria-label="Toggle confirm folder delete"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.confirmFolderDelete
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Auto-Complete Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="auto-complete" class="block font-medium text-gray-900">
              Auto-Complete Characters
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Automatically close brackets, quotes, and markdown formatting
            </p>
          </div>
            <button
              id="auto-complete"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.enableAutoComplete
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.enableAutoComplete = !settings.enableAutoComplete;
            }}
            aria-label="Toggle auto-complete characters"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.enableAutoComplete
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Show All Notes Folder Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="show-all-notes" class="block font-medium text-gray-900">
              Show "All Notes" Folder
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Display a special folder showing all notes
            </p>
          </div>
            <button
              id="show-all-notes"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.showAllNotesFolder
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.showAllNotesFolder = !settings.showAllNotesFolder;
            }}
            aria-label="Toggle show all notes folder"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.showAllNotesFolder
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Show Uncategorised Folder Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="show-uncategorised" class="block font-medium text-gray-900">
              Show "Uncategorised" Folder
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Display a special folder showing notes without a folder
            </p>
          </div>
            <button
              id="show-uncategorised"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.showUncategorisedFolder
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.showUncategorisedFolder = !settings.showUncategorisedFolder;
            }}
            aria-label="Toggle show uncategorised folder"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.showUncategorisedFolder
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Show Note Previews Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="show-previews" class="block font-medium text-gray-900">
              Show Note Previews
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Display preview text on note cards in the list
            </p>
          </div>
            <button
              id="show-previews"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.showNotePreviews
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.showNotePreviews = !settings.showNotePreviews;
            }}
            aria-label="Toggle show note previews"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.showNotePreviews
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Single Sidebar Mode Setting -->
        <div class="flex items-start gap-4">
          <div class="flex-1">
            <label for="single-sidebar" class="block font-medium text-gray-900">
              Single Sidebar Mode
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Combine folders and notes into a single sidebar. Disables "All Notes" and "Uncategorised" folders.
            </p>
          </div>
            <button
              id="single-sidebar"
              class="relative flex h-8 w-14 min-w-14 flex-none items-center rounded-full transition-colors p-0.5 {settings.singleSidebarMode
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.singleSidebarMode = !settings.singleSidebarMode;
            }}
            aria-label="Toggle single sidebar mode"
          >
            <span
              class="h-6 w-6 rounded-full bg-white transition-transform duration-200 ease-in-out {settings.singleSidebarMode
                ? 'translate-x-7'
                : 'translate-x-0'}"
            ></span>
          </button>
        </div>

        <!-- Shortcuts Setting -->
        <div class="border-t border-gray-200 pt-6">
          <h3 class="text-lg font-semibold text-gray-900">Shortcuts</h3>
          <p class="text-sm text-gray-500 mt-1 mb-4">
            Click a shortcut to reprogram it.
          </p>
          
          <div class="space-y-2">
            {#each Object.entries(settings.shortcuts) as [key, value]}
              <div class="flex items-center justify-between p-2 hover:bg-gray-50 rounded-lg group">
                <span class="text-sm font-medium text-gray-700 capitalize">
                  {key.replace(/([A-Z])/g, ' $1').trim()}
                </span>
                <button
                  class="px-3 py-1.5 rounded bg-gray-100 border border-gray-300 text-xs font-mono min-w-[100px] text-center hover:border-indigo-500 transition-all {recordingShortcut === key ? 'ring-2 ring-indigo-500 bg-white' : ''}"
                  onclick={() => recordingShortcut = (recordingShortcut === key ? null : key)}
                  onkeydown={handleShortcutKeyDown}
                >
                  {recordingShortcut === key ? 'Press keys...' : value}
                </button>
              </div>
            {/each}
          </div>
        </div>

        {#if isTauri}
          <div class="border-t border-gray-200 pt-6">
            <h3 class="text-lg font-semibold text-gray-900">Sync</h3>
            <p class="text-sm text-gray-500 mt-1">
              Configure a server URL to sync notes between devices.
            </p>

            <div class="mt-4 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-900" for="sync-server-url">
                  Server URL
                </label>
                <input
                  id="sync-server-url"
                  class="mt-1 w-full rounded-md border border-gray-300 px-3 py-2"
                  placeholder="https://notes.yourdomain.com"
                  bind:value={settings.syncServerUrl}
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-900" for="sync-username">
                  Username
                </label>
                <input
                  id="sync-username"
                  class="mt-1 w-full rounded-md border border-gray-300 px-3 py-2"
                  placeholder="mmccrum"
                  bind:value={settings.syncUsername}
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-900" for="sync-password">
                  Password
                </label>
                <input
                  id="sync-password"
                  type="password"
                  class="mt-1 w-full rounded-md border border-gray-300 px-3 py-2"
                  placeholder="(currently optional)"
                  bind:value={syncPassword}
                />
              </div>

              <div class="flex gap-2">
                <button
                  class="px-4 py-2 bg-gray-200 text-gray-900 rounded-lg hover:bg-gray-300 transition-colors disabled:opacity-60"
                  onclick={handleLogin}
                  disabled={loginBusy || !settings.syncServerUrl || !settings.syncUsername}
                >
                  {loginBusy ? 'Logging in…' : 'Login'}
                </button>
                <button
                  class="px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition-colors disabled:opacity-60"
                  onclick={handleSyncNow}
                  disabled={syncBusy || !settings.syncServerUrl}
                >
                  {syncBusy ? 'Syncing…' : 'Sync now'}
                </button>
              </div>

              <div class="text-sm text-gray-600">
                <div>Last sync: {settings.lastSync ?? 'never'}</div>
                {#if syncStatus}
                  <div class="mt-1">{syncStatus}</div>
                {/if}
              </div>
            </div>
          </div>
        {/if}

      </div>

      <div class="border-t border-gray-200 p-6 flex justify-end">
        <button
          class="px-6 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition-colors"
          onclick={() => {
            open = false;
          }}
        >
          Done
        </button>
      </div>
    </div>
  </div>
{/if}
