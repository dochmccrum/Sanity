<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import TitleBar from '$lib/components/TitleBar.svelte';
	const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="fixed inset-0 flex flex-col overflow-hidden bg-white text-gray-900 font-sans" style="transform: translateZ(0);">
	{#if isTauri}
		<div class="absolute inset-0 pointer-events-none ring-1 ring-gray-200 ring-inset z-[9999]"></div>
		<TitleBar />
	{/if}
	<div class="flex-1 overflow-hidden relative">
		{@render children()}
	</div>
</div>
