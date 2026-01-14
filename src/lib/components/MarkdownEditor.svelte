<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { EditorState } from '@codemirror/state';
  import { markdown } from '@codemirror/lang-markdown';
  import { oneDark } from '@codemirror/theme-one-dark';

  interface Props {
    value: string;
    onChange?: (value: string) => void;
    placeholder?: string;
    readonly?: boolean;
  }

  let { value = '', onChange, placeholder = 'Start writing...', readonly = false }: Props = $props();

  let editorContainer: HTMLDivElement;
  let view: EditorView | null = null;

  onMount(() => {
    if (!editorContainer) return;

    const startState = EditorState.create({
      doc: value,
      extensions: [
        basicSetup,
        markdown(),
        oneDark,
        EditorView.updateListener.of((update) => {
          if (update.docChanged && onChange) {
            const newValue = update.state.doc.toString();
            onChange(newValue);
          }
        }),
        EditorView.editable.of(!readonly),
        EditorState.readOnly.of(readonly),
      ],
    });

    view = new EditorView({
      state: startState,
      parent: editorContainer,
    });
  });

  onDestroy(() => {
    if (view) {
      view.destroy();
      view = null;
    }
  });

  // Update editor content when value prop changes externally
  $effect(() => {
    if (view && value !== view.state.doc.toString()) {
      view.dispatch({
        changes: {
          from: 0,
          to: view.state.doc.length,
          insert: value,
        },
      });
    }
  });
</script>

<div class="markdown-editor">
  <div bind:this={editorContainer} class="editor-container"></div>
</div>

<style>
  .markdown-editor {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .editor-container {
    flex: 1;
    overflow: auto;
  }

  .editor-container :global(.cm-editor) {
    height: 100%;
    font-size: 14px;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  }

  .editor-container :global(.cm-scroller) {
    overflow: auto;
  }

  .editor-container :global(.cm-content) {
    padding: 16px;
    min-height: 100%;
  }
</style>
