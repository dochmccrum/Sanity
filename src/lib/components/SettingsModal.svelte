<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  let { open = $bindable(false), settings }: { open?: boolean; settings: any } = $props();

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      open = false;
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 flex items-center justify-center z-50" style="background-color: rgba(0, 0, 0, 0.1);" onmousedown={handleBackdropClick} transition:fade={{ duration: 200 }}>
    <div class="bg-white rounded-lg shadow-xl max-w-md w-full mx-4" transition:scale={{ duration: 200, start: 0.95 }}>
      <div class="border-b border-gray-200 p-6">
        <h2 class="text-2xl font-bold text-gray-900">Settings</h2>
      </div>

      <div class="p-6 space-y-6">
        <!-- Auto-focus Title Setting -->
        <div class="flex items-center justify-between">
          <div>
            <label for="auto-focus" class="block font-medium text-gray-900">
              Auto-focus Title
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Automatically focus the title field when creating a new note
            </p>
          </div>
          <button
            id="auto-focus"
            class="relative inline-flex h-8 w-14 items-center rounded-full transition-colors {settings.autoFocusTitleOnNewNote
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.autoFocusTitleOnNewNote = !settings.autoFocusTitleOnNewNote;
            }}
            aria-label="Toggle auto-focus title"
          >
            <span
              class="inline-block h-6 w-6 transform rounded-full bg-white transition-transform {settings.autoFocusTitleOnNewNote
                ? 'translate-x-7'
                : 'translate-x-1'}"
            ></span>
          </button>
        </div>

        <!-- Confirm Before Delete Setting -->
        <div class="flex items-center justify-between">
          <div>
            <label for="confirm-delete" class="block font-medium text-gray-900">
              Confirm Before Delete
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Show confirmation dialog before deleting notes
            </p>
          </div>
          <button
            id="confirm-delete"
            class="relative inline-flex h-8 w-14 items-center rounded-full transition-colors {settings.confirmBeforeDelete
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.confirmBeforeDelete = !settings.confirmBeforeDelete;
            }}
            aria-label="Toggle confirm before delete"
          >
            <span
              class="inline-block h-6 w-6 transform rounded-full bg-white transition-transform {settings.confirmBeforeDelete
                ? 'translate-x-7'
                : 'translate-x-1'}"
            ></span>
          </button>
        </div>

        <!-- Auto-Complete Setting -->
        <div class="flex items-center justify-between">
          <div>
            <label for="auto-complete" class="block font-medium text-gray-900">
              Auto-Complete Characters
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Automatically close brackets, quotes, and markdown formatting
            </p>
          </div>
          <button
            id="auto-complete"
            class="relative inline-flex h-8 w-14 items-center rounded-full transition-colors {settings.enableAutoComplete
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.enableAutoComplete = !settings.enableAutoComplete;
            }}
            aria-label="Toggle auto-complete characters"
          >
            <span
              class="inline-block h-6 w-6 transform rounded-full bg-white transition-transform {settings.enableAutoComplete
                ? 'translate-x-7'
                : 'translate-x-1'}"
            ></span>
          </button>
        </div>

        <!-- Show All Notes Folder Setting -->
        <div class="flex items-center justify-between">
          <div>
            <label for="show-all-notes" class="block font-medium text-gray-900">
              Show "All Notes" Folder
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Display a special folder showing all notes
            </p>
          </div>
          <button
            id="show-all-notes"
            class="relative inline-flex h-8 w-14 items-center rounded-full transition-colors {settings.showAllNotesFolder
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.showAllNotesFolder = !settings.showAllNotesFolder;
            }}
            aria-label="Toggle show all notes folder"
          >
            <span
              class="inline-block h-6 w-6 transform rounded-full bg-white transition-transform {settings.showAllNotesFolder
                ? 'translate-x-7'
                : 'translate-x-1'}"
            ></span>
          </button>
        </div>

        <!-- Show Uncategorised Folder Setting -->
        <div class="flex items-center justify-between">
          <div>
            <label for="show-uncategorised" class="block font-medium text-gray-900">
              Show "Uncategorised" Folder
            </label>
            <p class="text-sm text-gray-500 mt-1">
              Display a special folder showing notes without a folder
            </p>
          </div>
          <button
            id="show-uncategorised"
            class="relative inline-flex h-8 w-14 items-center rounded-full transition-colors {settings.showUncategorisedFolder
              ? 'bg-indigo-600'
              : 'bg-gray-300'}"
            onclick={() => {
              settings.showUncategorisedFolder = !settings.showUncategorisedFolder;
            }}
            aria-label="Toggle show uncategorised folder"
          >
            <span
              class="inline-block h-6 w-6 transform rounded-full bg-white transition-transform {settings.showUncategorisedFolder
                ? 'translate-x-7'
                : 'translate-x-1'}"
            ></span>
          </button>
        </div>

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
