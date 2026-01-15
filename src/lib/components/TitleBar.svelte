<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  
  // We need to keep track of maximized state to toggle the icon
  let isMaximized = $state(false);
  let appWindow: any = null;
  let unlistenFn: any = null;

  async function checkMaximized() {
    if (!appWindow) return;
    isMaximized = await appWindow.isMaximized();
  }

  onMount(() => {
    const initWindow = async () => {
      try {
        appWindow = getCurrentWindow();
        console.log('Window initialized:', appWindow);
        
        // Check initial state
        await checkMaximized();
        
        // Listen for resize events to update the icon if the user snaps the window
        unlistenFn = await appWindow.listen('tauri://resize', checkMaximized);
      } catch (error) {
        console.error('Failed to initialize window controls:', error);
      }
    };
    
    initWindow();
    
    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  });

  function minimize() {
    console.log('Minimize clicked', appWindow);
    if (appWindow) {
      appWindow.minimize();
    }
  }

  function maximize() {
    console.log('Maximize clicked', appWindow);
    if (appWindow) {
      appWindow.toggleMaximize();
      checkMaximized();
    }
  }

  function close() {
    console.log('Close clicked', appWindow);
    if (appWindow) {
      appWindow.close();
    }
  }
</script>

<div class="h-8 bg-gray-100 border-b border-gray-200 flex justify-between items-center select-none w-full shrink-0 z-50">
  <div data-tauri-drag-region class="flex items-center pl-4 gap-2 flex-1 h-full">
     <span class="text-xs font-medium text-gray-500">Sanity</span>
  </div>
  
  <div class="flex h-full">
    <button onclick={minimize} class="inline-flex content-center items-center justify-center w-10 hover:bg-gray-200 text-gray-600 transition-colors" aria-label="Minimize">
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
    </button>
    
    <button onclick={maximize} class="inline-flex content-center items-center justify-center w-10 hover:bg-gray-200 text-gray-600 transition-colors" aria-label="Maximize">
      {#if isMaximized}
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="11" width="10" height="10" rx="1" ry="1"></rect>
          <path d="M11 3a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-2"></path>
          <line x1="11" y1="11" x2="11" y2="3"></line> <!-- Connect the boxes visually -->
        </svg>
      {:else}
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
        </svg>
      {/if}
    </button>
    
    <button onclick={close} class="inline-flex content-center items-center justify-center w-10 hover:bg-red-500 hover:text-white text-gray-600 transition-colors" aria-label="Close">
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"></line>
        <line x1="6" y1="6" x2="18" y2="18"></line>
      </svg>
    </button>
  </div>
</div>
