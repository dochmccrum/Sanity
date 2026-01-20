<script lang="ts">
  /**
   * CollaborativeEditor - TipTap editor with Yjs CRDT integration
   * 
   * This editor uses Yjs for conflict-free collaborative editing.
   * All changes are automatically synced across devices without data loss.
   */
  import { onMount, onDestroy } from 'svelte';
  import { browser } from '$app/environment';
  import { Editor, Extension } from '@tiptap/core';
  import { Plugin, PluginKey } from '@tiptap/pm/state';
  import { Decoration, DecorationSet } from '@tiptap/pm/view';
  import StarterKit from '@tiptap/starter-kit';
  import Code from '@tiptap/extension-code';
  import Placeholder from '@tiptap/extension-placeholder';
  import Typography from '@tiptap/extension-typography';
  import TaskList from '@tiptap/extension-task-list';
  import TaskItem from '@tiptap/extension-task-item';
  import Image from '@tiptap/extension-image';
  import Collaboration from '@tiptap/extension-collaboration';
  import { Markdown } from '@tiptap/markdown';
  import * as Y from 'yjs';
  import katex from 'katex';
  import 'katex/dist/katex.min.css';
  import './WysiwygEditor.css';
  import { uploadImage, uploadImageFromClipboard } from '$lib/utils/imageUpload';

  interface Props {
    noteId: string;
    ydoc: Y.Doc;
    initialContent?: string;
    onchange?: (content: string) => void;
    enableAutoComplete?: boolean;
  }

  let { 
    noteId, 
    ydoc, 
    initialContent = '', 
    onchange, 
    enableAutoComplete = true 
  }: Props = $props();

  let editorElement: HTMLDivElement;
  let editor: Editor | null = null;
  let lastNoteId = '';

  export function focus() {
    if (editor) {
      editor.commands.focus();
    }
  }

  export function insertImage(src: string) {
    if (editor) {
      editor.chain()
        .focus()
        .insertContent({
          type: 'image',
          attrs: { src }
        })
        .run();
    }
  }

  function handleAutoComplete(view: any, event: KeyboardEvent): boolean {
    if (!enableAutoComplete) return false;
    
    const key = event.key;
    const { state, dispatch } = view;
    const { from, to } = state.selection;
    
    if (from !== to) return false;

    const pairs: Record<string, string> = {
      '[': ']',
      '{': '}',
      '(': ')',
      '"': '"',
      '`': '`'
    };

    if (!pairs[key]) return false;

    const beforeText = state.doc.textBetween(Math.max(0, from - 5), from);
    const afterText = state.doc.textBetween(from, Math.min(state.doc.content.size, from + 5));
    const nextChar = afterText.charAt(0);
    
    if (nextChar === pairs[key]) {
      event.preventDefault();
      const newPos = from + 1;
      dispatch(state.tr.setSelection(state.selection.constructor.near(state.doc.resolve(newPos))));
      return true;
    }

    event.preventDefault();
    const closing = pairs[key];
    const insertText = key + closing;
    const tr = state.tr.insertText(insertText, from, to);
    tr.setSelection(state.selection.constructor.near(tr.doc.resolve(from + 1)));
    dispatch(tr);
    return true;
  }

  // Math rendering plugin
  const MathPlugin = Extension.create({
    name: 'mathPlugin',

    addProseMirrorPlugins() {
      let updateTimeout: any = null;

      return [
        new Plugin({
          key: new PluginKey('mathRender'),
          state: {
            init: () => DecorationSet.empty,
            apply: (tr: any, value: any) => {
              if (updateTimeout) clearTimeout(updateTimeout);
              updateTimeout = setTimeout(() => {}, 0);
              return value.map(tr.mapping, tr.doc);
            }
          },
          props: {
            decorations(state: any) {
              const decorations: any[] = [];
              const { from: selFrom, to: selTo } = state.selection;
              
              state.doc.descendants((node: any, pos: number) => {
                if (!node.isText) return;
                
                const text = node.text;
                if (!text) return;

                // Block math: $$...$$
                const blockRegex = /\$\$([^\$\n][^\$]*?[^\$\n]|[^\$\n])\$\$/g;
                let blockMatch;
                const usedRanges: Array<[number, number]> = [];

                while ((blockMatch = blockRegex.exec(text)) !== null) {
                  const content = blockMatch[1];
                  const startPos = pos + blockMatch.index;
                  const endPos = startPos + blockMatch[0].length;
                  usedRanges.push([startPos, endPos]);

                  try {
                    const html = katex.renderToString(content, { displayMode: true, throwOnError: true });
                    const div = document.createElement('div');
                    div.className = 'math-block-render';
                    div.innerHTML = html;
                    
                    decorations.push(
                      Decoration.widget(startPos, () => div, { side: -1 })
                    );
                    
                    const isCursorInside = (selFrom >= startPos && selFrom <= endPos) || (selTo >= startPos && selTo <= endPos);
                    const className = isCursorInside ? 'math-hide math-hide-active' : 'math-hide';
                    
                    decorations.push(
                      Decoration.inline(startPos, endPos, { class: className })
                    );
                  } catch (e) {
                    // Silent fail
                  }
                }

                // Inline math: $...$
                const inlineRegex = /\$([^\$\n]+?)\$/g;
                let inlineMatch;

                while ((inlineMatch = inlineRegex.exec(text)) !== null) {
                  const content = inlineMatch[1];
                  const startPos = pos + inlineMatch.index;
                  const endPos = startPos + inlineMatch[0].length;
                  
                  const isInBlock = usedRanges.some(([s, e]) => startPos >= s && endPos <= e);
                  if (isInBlock) continue;

                  try {
                    const html = katex.renderToString(content, { throwOnError: true });
                    const span = document.createElement('span');
                    span.className = 'math-inline-render';
                    span.innerHTML = html;
                    
                    decorations.push(
                      Decoration.widget(startPos, () => span, { side: -1 })
                    );
                    
                    const isCursorInside = (selFrom >= startPos && selFrom <= endPos) || (selTo >= startPos && selTo <= endPos);
                    const className = isCursorInside ? 'math-hide math-hide-active' : 'math-hide';
                    
                    decorations.push(
                      Decoration.inline(startPos, endPos, { class: className })
                    );
                  } catch (e) {
                    // Silent fail
                  }
                }
              });

              return DecorationSet.create(state.doc, decorations);
            }
          }
        })
      ];
    }
  });

  function createEditor() {
    if (!browser || !editorElement || !ydoc) return;

    // Get the XmlFragment from the Y.Doc
    const fragment = ydoc.getXmlFragment('content');
    
    // Check if we need to initialize with content
    const meta = ydoc.getMap('meta');
    const needsInit = fragment.length === 0 && initialContent;

    editor = new Editor({
      element: editorElement,
      extensions: [
        StarterKit.configure({
          history: false,
          heading: { levels: [1, 2, 3] },
          bulletList: { keepMarks: true, keepAttributes: false },
          orderedList: { keepMarks: true, keepAttributes: false },
          listItem: { HTMLAttributes: { class: 'list-item' } },
        }),
        // Enable Yjs collaboration
        Collaboration.configure({
          document: ydoc,
          field: 'content',
        }),
        Code.configure({
          HTMLAttributes: { class: 'inline-code' }
        }),
        Image.configure({
          inline: true,
          allowBase64: true,
          HTMLAttributes: { class: 'editor-image' }
        }),
        TaskList,
        TaskItem.configure({ nested: true }),
        Placeholder.configure({
          placeholder: 'Start typing... Use ** for bold, _ for italic, ` for code, $x+y$ for equations. Drag or paste images...'
        }),
        Typography,
        Markdown,
        MathPlugin
      ],
      // Don't set content directly - Yjs handles it
      content: needsInit ? initialContent : undefined,
      editorProps: {
        attributes: {
          class: 'prose prose-sm sm:prose lg:prose-lg xl:prose-xl focus:outline-none max-w-none p-6'
        },
        handleDrop: (view, event, slice, moved) => {
          if (moved) return false;
          
          const files = event.dataTransfer?.files;
          if (!files || files.length === 0) return false;
          
          const imageFiles = Array.from(files).filter(file => file.type.startsWith('image/'));
          if (imageFiles.length === 0) return false;
          
          event.preventDefault();
          event.stopPropagation();
          
          const coordinates = view.posAtCoords({
            left: event.clientX,
            top: event.clientY
          });
          
          (async () => {
            for (const file of imageFiles) {
              try {
                const uri = await uploadImage(file);
                if (editor && coordinates) {
                  editor.chain()
                    .focus()
                    .insertContentAt(coordinates.pos, {
                      type: 'image',
                      attrs: { src: uri }
                    })
                    .run();
                }
              } catch (error) {
                console.error('Failed to upload dropped image:', error);
              }
            }
          })();
          
          return true;
        },
        handlePaste: (view, event) => {
          const files = event.clipboardData?.files;
          if (!files || files.length === 0) return false;
          
          const imageFiles = Array.from(files).filter(file => file.type.startsWith('image/'));
          if (imageFiles.length === 0) return false;
          
          event.preventDefault();
          event.stopPropagation();
          
          (async () => {
            for (const file of imageFiles) {
              try {
                const blob = file as Blob;
                const uri = await uploadImageFromClipboard(blob);
                
                if (editor) {
                  editor.chain()
                    .focus()
                    .insertContent({
                      type: 'image',
                      attrs: { src: uri }
                    })
                    .run();
                }
              } catch (error) {
                console.error('Failed to upload pasted image:', error);
              }
            }
          })();
          
          return true;
        },
        handleKeyDown: (view, event) => {
          if (event.key === 'Backspace') {
            const { state } = view;
            const { selection } = state;
            const from = selection.$from;
            
            if (from.parentOffset === 0) {
              if (from.parent.type.name === 'paragraph' && from.parent.textContent === '') {
                const depth = from.depth;
                if (depth >= 2) {
                  const grandparent = from.node(depth - 1);
                  if (grandparent.type.name === 'listItem' || grandparent.type.name === 'taskItem') {
                    const itemType = grandparent.type.name === 'taskItem' ? 'taskItem' : 'listItem';
                    if (editor?.commands.liftListItem(itemType)) {
                      event.preventDefault();
                      return true;
                    }
                  }
                }
              }
            }
          }
          
          return handleAutoComplete(view, event);
        }
      },
      onUpdate: ({ editor }) => {
        // Get HTML content for callbacks (e.g., for title extraction)
        const html = editor.getHTML();
        onchange?.(html);
      }
    });

    lastNoteId = noteId;
  }

  onMount(() => {
    createEditor();
  });

  // Handle note changes - recreate editor with new Y.Doc
  $effect(() => {
    if (browser && noteId && noteId !== lastNoteId && ydoc) {
      // Destroy old editor
      if (editor) {
        editor.destroy();
        editor = null;
      }
      // Create new editor with new Y.Doc
      createEditor();
    }
  });

  onDestroy(() => {
    if (editor) {
      editor.destroy();
    }
  });
</script>

<div bind:this={editorElement} class="tiptap-editor collaborative-editor"></div>

<style>
  .collaborative-editor {
    position: relative;
  }
  
  /* Remote cursor styles (for future awareness integration) */
  :global(.collaboration-cursor__caret) {
    border-left: 1px solid #0d0d0d;
    border-right: 1px solid #0d0d0d;
    margin-left: -1px;
    margin-right: -1px;
    pointer-events: none;
    position: relative;
    word-break: normal;
  }

  :global(.collaboration-cursor__label) {
    border-radius: 3px 3px 3px 0;
    color: #0d0d0d;
    font-size: 12px;
    font-style: normal;
    font-weight: 600;
    left: -1px;
    line-height: normal;
    padding: 0.1rem 0.3rem;
    position: absolute;
    top: -1.4em;
    user-select: none;
    white-space: nowrap;
  }
</style>
