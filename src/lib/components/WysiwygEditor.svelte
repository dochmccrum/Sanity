<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { browser } from '$app/environment';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import Placeholder from '@tiptap/extension-placeholder';
  import Typography from '@tiptap/extension-typography';
  
  let { value = $bindable(''), onchange }: { value?: string; onchange?: (val: string) => void } = $props();
  
  let editorElement: HTMLDivElement;
  let editor: Editor | null = null;
  let isUpdating = false;
  
  onMount(() => {
    if (browser && editorElement) {
      editor = new Editor({
        element: editorElement,
        extensions: [
          StarterKit.configure({
            heading: {
              levels: [1, 2, 3]
            }
          }),
          Placeholder.configure({
            placeholder: 'Start typing... Use ## for headings, ** for bold, * for italic, - for lists...'
          }),
          Typography
        ],
        content: value,
        editorProps: {
          attributes: {
            class: 'prose prose-sm sm:prose lg:prose-lg xl:prose-xl focus:outline-none max-w-none p-6'
          }
        },
        onUpdate: ({ editor }) => {
          if (!isUpdating) {
            const markdown = editor.storage.markdown?.getMarkdown() || editor.getText();
            value = markdown;
            onchange?.(markdown);
          }
        }
      });
    }
  });
  
  // Update editor when value changes externally
  $effect(() => {
    if (editor && value !== editor.getText()) {
      isUpdating = true;
      editor.commands.setContent(value);
      isUpdating = false;
    }
  });
  
  onDestroy(() => {
    if (editor) {
      editor.destroy();
    }
  });
</script>

<div bind:this={editorElement} class="tiptap-editor"></div>

<style>
  .tiptap-editor {
    height: 100%;
    overflow-y: auto;
  }
  
  :global(.tiptap-editor .ProseMirror) {
    height: 100%;
    min-height: 100%;
    outline: none;
  }
  
  :global(.tiptap-editor h1) {
    font-size: 2.25em;
    font-weight: 800;
    margin-top: 0.67em;
    margin-bottom: 0.67em;
    line-height: 1.2;
  }
  
  :global(.tiptap-editor h2) {
    font-size: 1.875em;
    font-weight: 700;
    margin-top: 0.83em;
    margin-bottom: 0.83em;
    line-height: 1.3;
  }
  
  :global(.tiptap-editor h3) {
    font-size: 1.5em;
    font-weight: 600;
    margin-top: 1em;
    margin-bottom: 1em;
    line-height: 1.4;
  }
  
  :global(.tiptap-editor p) {
    margin-top: 1em;
    margin-bottom: 1em;
    line-height: 1.75;
  }
  
  :global(.tiptap-editor strong) {
    font-weight: 700;
  }
  
  :global(.tiptap-editor em) {
    font-style: italic;
  }
  
  :global(.tiptap-editor code) {
    background: #f3f4f6;
    padding: 0.2em 0.4em;
    border-radius: 0.25em;
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
    font-size: 0.875em;
  }
  
  :global(.tiptap-editor pre) {
    background: #1e293b;
    color: #e2e8f0;
    padding: 1em;
    border-radius: 0.5em;
    overflow-x: auto;
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
  }
  
  :global(.tiptap-editor pre code) {
    background: transparent;
    padding: 0;
    color: inherit;
  }
  
  :global(.tiptap-editor blockquote) {
    border-left: 4px solid #6366f1;
    padding-left: 1em;
    margin: 1.5em 0;
    color: #64748b;
    font-style: italic;
  }
  
  :global(.tiptap-editor ul, .tiptap-editor ol) {
    padding-left: 2em;
    margin: 1em 0;
  }
  
  :global(.tiptap-editor li) {
    margin: 0.5em 0;
  }
  
  :global(.tiptap-editor hr) {
    border: none;
    border-top: 2px solid #e5e7eb;
    margin: 2em 0;
  }
  
  :global(.tiptap-editor a) {
    color: #6366f1;
    text-decoration: underline;
  }
  
  :global(.tiptap-editor .ProseMirror p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    color: #adb5bd;
    pointer-events: none;
    height: 0;
  }
</style>
