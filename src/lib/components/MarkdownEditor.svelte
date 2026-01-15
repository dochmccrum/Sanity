<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { EditorState } from '@codemirror/state';
  import { markdown } from '@codemirror/lang-markdown';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { uploadImage, uploadImageFromClipboard } from '$lib/utils/imageUpload';

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
        // Image drop/paste support
        EditorView.domEventHandlers({
          drop: (event, view) => {
            if (readonly) return false;
            
            const files = event.dataTransfer?.files;
            if (!files || files.length === 0) return false;
            
            const imageFiles = Array.from(files).filter(file => file.type.startsWith('image/'));
            if (imageFiles.length === 0) return false;
            
            event.preventDefault();
            
            // Get cursor position from coordinates
            const coords = view.posAtCoords({ x: event.clientX, y: event.clientY });
            const pos = coords ?? view.state.selection.main.head;
            
            // Upload each image and insert markdown asynchronously
            imageFiles.forEach(async (file) => {
              try {
                const uri = await uploadImage(file);
                const markdown = `![${file.name}](${uri})\n`;
                
                view.dispatch({
                  changes: { from: pos, insert: markdown }
                });
              } catch (error) {
                console.error('Failed to upload dropped image:', error);
              }
            });
            
            return true;
          },
          paste: (event, view) => {
            if (readonly) return false;
            
            const files = event.clipboardData?.files;
            if (!files || files.length === 0) return false;
            
            const imageFiles = Array.from(files).filter(file => file.type.startsWith('image/'));
            if (imageFiles.length === 0) return false;
            
            event.preventDefault();
            
            // Get cursor position
            const pos = view.state.selection.main.head;
            
            // Upload each image and insert markdown asynchronously
            imageFiles.forEach(async (file) => {
              try {
                const blob = file as Blob;
                const uri = await uploadImageFromClipboard(blob);
                const markdown = `![pasted-image](${uri})\n`;
                
                view.dispatch({
                  changes: { from: pos, insert: markdown }
                });
              } catch (error) {
                console.error('Failed to upload pasted image:', error);
              }
            });
            
            return true;
          }
        })
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
