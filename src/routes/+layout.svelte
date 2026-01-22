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

<div class="flex flex-col h-screen overflow-hidden bg-white text-gray-900 font-sans relative">
	{#if isTauri}
		<div class="absolute inset-0 pointer-events-none border border-gray-200 z-[9999]"></div>
		<TitleBar />
	{/if}
	<div class="flex-1 overflow-hidden relative">
		{@render children()}
	</div>
</div>
