<script lang="ts">
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
  import { Markdown } from '@tiptap/markdown';
  import katex from 'katex';
  import 'katex/dist/katex.min.css';
  import './WysiwygEditor.css';
  import { uploadImage, uploadImageFromClipboard } from '$lib/utils/imageUpload';

  let { value = '', onchange, noteId, enableAutoComplete = true }: { value?: string; onchange?: (val: string) => void; noteId?: string; enableAutoComplete?: boolean } = $props();

  let editorElement: HTMLDivElement;
  let editor: Editor | null = null;
  let isUpdating = false;
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
    
    // Don't auto-complete if there's selected text
    if (from !== to) return false;

    // Map of opening character to closing character
    const pairs: Record<string, string> = {
      '[': ']',
      '{': '}',
      '(': ')',
      '"': '"',
      '`': '`'
    };

    if (!pairs[key]) return false;

    // Get text before and after cursor to check context
    const beforeText = state.doc.textBetween(Math.max(0, from - 5), from);
    const afterText = state.doc.textBetween(from, Math.min(state.doc.content.size, from + 5));
    
    // Check what character is after the cursor
    const nextChar = afterText.charAt(0);
    
    // If the next character is already the closing pair, just move cursor past it
    if (nextChar === pairs[key]) {
      event.preventDefault();
      const newPos = from + 1;
      dispatch(state.tr.setSelection(state.selection.constructor.near(state.doc.resolve(newPos))));
      return true;
    }

    // Prevent default key handling and insert pair with cursor positioned between
    event.preventDefault();
    const closing = pairs[key];
    const insertText = key + closing;
    const tr = state.tr.insertText(insertText, from, to);
    tr.setSelection(state.selection.constructor.near(tr.doc.resolve(from + 1)));
    dispatch(tr);
    return true;
  }

  // Simple math rendering plugin using decorations
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
              
              updateTimeout = setTimeout(() => {
                // Force a redecorate on next state check
              }, 0);
              
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
                    
                    // Check if cursor is inside this math expression
                    const isCursorInside = (selFrom >= startPos && selFrom <= endPos) || (selTo >= startPos && selTo <= endPos);
                    const className = isCursorInside ? 'math-hide math-hide-active' : 'math-hide';
                    
                    // Hide the original text
                    decorations.push(
                      Decoration.inline(startPos, endPos, { class: className })
                    );
                  } catch (e) {
                    // Silent fail
                  }
                }

                // Inline math: $...$ (not inside $$...$$)
                const inlineRegex = /\$([^\$\n]+?)\$/g;
                let inlineMatch;

                while ((inlineMatch = inlineRegex.exec(text)) !== null) {
                  const content = inlineMatch[1];
                  const startPos = pos + inlineMatch.index;
                  const endPos = startPos + inlineMatch[0].length;
                  
                  // Skip if inside block math
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
                    
                    // Check if cursor is inside this math expression
                    const isCursorInside = (selFrom >= startPos && selFrom <= endPos) || (selTo >= startPos && selTo <= endPos);
                    const className = isCursorInside ? 'math-hide math-hide-active' : 'math-hide';
                    
                    // Hide the original text
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

  onMount(() => {
    if (browser && editorElement) {
      editor = new Editor({
        element: editorElement,
        extensions: [
          StarterKit.configure({
            heading: {
              levels: [1, 2, 3]
            },
            bulletList: {
              keepMarks: true,
              keepAttributes: false,
            },
            orderedList: {
              keepMarks: true,
              keepAttributes: false,
            },
            listItem: {
              HTMLAttributes: {
                class: 'list-item'
              }
            }
          }),
          Code.configure({
            HTMLAttributes: {
              class: 'inline-code'
            }
          }),
          Image.configure({
            inline: true,
            allowBase64: true,
            HTMLAttributes: {
              class: 'editor-image'
            }
          }),
          TaskList,
          TaskItem.configure({
            nested: true,
          }),
          Placeholder.configure({
            placeholder: 'Start typing... Use ** for bold, _ for italic, ` for code, $x+y$ for equations. Drag or paste images...'
          }),
          Typography,
          Markdown,
          MathPlugin
        ],
        content: value,
        editorProps: {
          attributes: {
            class: 'prose prose-sm sm:prose lg:prose-lg xl:prose-xl focus:outline-none max-w-none p-6'
          },
          handleDrop: (view, event, slice, moved) => {
            // Handle image drops - don't process if content is being moved within editor
            if (moved) return false;
            
            const files = event.dataTransfer?.files;
            console.log('Drop event:', { files: files?.length, types: event.dataTransfer?.types });
            
            if (!files || files.length === 0) return false;
            
            const imageFiles = Array.from(files).filter(file => file.type.startsWith('image/'));
            console.log('Image files found:', imageFiles.length);
            
            if (imageFiles.length === 0) return false;
            
            // Prevent default to handle the drop ourselves
            event.preventDefault();
            event.stopPropagation();
            
            // Get drop position
            const coordinates = view.posAtCoords({
              left: event.clientX,
              top: event.clientY
            });
            
            console.log('Uploading images...');
            
            // Upload and insert each image sequentially
            (async () => {
              for (const file of imageFiles) {
                try {
                  console.log('Uploading:', file.name);
                  const uri = await uploadImage(file);
                  console.log('Upload successful, URI:', uri);
                  
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
            // Handle pasted images
            const files = event.clipboardData?.files;
            if (!files || files.length === 0) return false;
            
            const imageFiles = Array.from(files).filter(file => file.type.startsWith('image/'));
            if (imageFiles.length === 0) return false;
            
            event.preventDefault();
            event.stopPropagation();
            
            // Upload and insert each image sequentially
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
            // Handle Backspace at the beginning of an empty list item to exit the list
            if (event.key === 'Backspace') {
              const { state } = view;
              const { selection } = state;
              const from = selection.$from;
              
              // Check if we're at the very start of the current node (offset 0)
              if (from.parentOffset === 0) {
                // Check if parent is a paragraph and if it's empty
                if (from.parent.type.name === 'paragraph' && from.parent.textContent === '') {
                  // Check if the paragraph's parent is a list item
                  const depth = from.depth;
                  if (depth >= 2) {
                    const grandparent = from.node(depth - 1);
                    if (grandparent.type.name === 'listItem' || grandparent.type.name === 'taskItem') {
                      // We're in an empty paragraph inside a list item - lift it out
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
          if (!isUpdating) {
            const html = editor.getHTML();
            onchange?.(html);
          }
        }
      });
      lastNoteId = noteId || '';
    }
  });

  // Only update editor content when switching to a DIFFERENT note, not on every keystroke
  $effect(() => {
    if (editor && noteId && noteId !== lastNoteId) {
      isUpdating = true;
      editor.commands.setContent(value);
      lastNoteId = noteId;
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
