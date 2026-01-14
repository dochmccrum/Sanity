<script lang="ts">
  import { onMount } from 'svelte';
  import katex from 'katex';
  import 'katex/dist/katex.min.css';

  interface Props {
    markdown: string;
  }

  let { markdown = '' }: Props = $props();

  let previewContainer: HTMLDivElement;

  function renderMarkdown(md: string): string {
    if (!md) return '<p class="text-gray-400 italic">Nothing to preview</p>';

    // Simple markdown rendering (you can replace this with a full markdown library like marked.js)
    let html = md
      // Headers
      .replace(/^### (.*$)/gim, '<h3>$1</h3>')
      .replace(/^## (.*$)/gim, '<h2>$1</h2>')
      .replace(/^# (.*$)/gim, '<h1>$1</h1>')
      // Bold
      .replace(/\*\*(.*?)\*\*/gim, '<strong>$1</strong>')
      // Italic
      .replace(/\*(.*?)\*/gim, '<em>$1</em>')
      // Code blocks
      .replace(/```(.*?)```/gis, '<pre><code>$1</code></pre>')
      // Inline code
      .replace(/`(.*?)`/gim, '<code>$1</code>')
      // Links
      .replace(/\[([^\]]+)\]\(([^)]+)\)/gim, '<a href="$2" target="_blank" rel="noopener">$1</a>')
      // Line breaks
      .replace(/\n/gim, '<br>');

    // Render LaTeX equations with KaTeX
    // Display equations: $$...$$
    html = html.replace(/\$\$(.*?)\$\$/gs, (match, equation) => {
      try {
        return katex.renderToString(equation, { displayMode: true, throwOnError: false });
      } catch (err) {
        return `<span class="text-red-500">LaTeX Error: ${equation}</span>`;
      }
    });

    // Inline equations: $...$
    html = html.replace(/\$([^\$]+)\$/g, (match, equation) => {
      try {
        return katex.renderToString(equation, { displayMode: false, throwOnError: false });
      } catch (err) {
        return `<span class="text-red-500">LaTeX Error: ${equation}</span>`;
      }
    });

    return html;
  }

  $effect(() => {
    if (previewContainer) {
      previewContainer.innerHTML = renderMarkdown(markdown);
    }
  });
</script>

<div class="markdown-preview">
  <div bind:this={previewContainer} class="preview-content"></div>
</div>

<style>
  .markdown-preview {
    height: 100%;
    overflow: auto;
    padding: 16px;
    background: #1e1e1e;
    color: #d4d4d4;
  }

  .preview-content :global(h1) {
    font-size: 2em;
    font-weight: bold;
    margin-top: 0.5em;
    margin-bottom: 0.5em;
  }

  .preview-content :global(h2) {
    font-size: 1.5em;
    font-weight: bold;
    margin-top: 0.5em;
    margin-bottom: 0.5em;
  }

  .preview-content :global(h3) {
    font-size: 1.25em;
    font-weight: bold;
    margin-top: 0.5em;
    margin-bottom: 0.5em;
  }

  .preview-content :global(strong) {
    font-weight: bold;
  }

  .preview-content :global(em) {
    font-style: italic;
  }

  .preview-content :global(code) {
    background: #2d2d2d;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 0.9em;
  }

  .preview-content :global(pre) {
    background: #2d2d2d;
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 12px 0;
  }

  .preview-content :global(pre code) {
    background: none;
    padding: 0;
  }

  .preview-content :global(a) {
    color: #4a9eff;
    text-decoration: underline;
  }

  .preview-content :global(a:hover) {
    color: #6bb3ff;
  }

  /* KaTeX styles */
  .preview-content :global(.katex-display) {
    margin: 1em 0;
  }
</style>
