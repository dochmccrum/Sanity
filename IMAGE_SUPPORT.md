# Image Support Implementation

This document describes the image support added to the JFNotes application.

## Features Added

1. **Automatic Image Scaling**
   - Images are automatically scaled to max 800x600 while maintaining aspect ratio
   - Responsive design scales to 100% width on smaller screens

2. **Multiple Upload Methods**
   - **Drag and Drop**: Drag image files directly into the editor
   - **Paste**: Paste images from clipboard (Ctrl+V / Cmd+V)

3. **Storage**
   - Images are stored in the `.assets` folder in the app data directory
   - Images are embedded as asset:// URIs, not separate files in the note
   - Auto-scaled before storage to save space

4. **Editor Support**
   - WYSIWYG Editor: Full image support with inline display
   - Markdown Editor: Images inserted as markdown `![](uri)` syntax
   - Markdown Preview: Images rendered with proper styling

## Implementation Details

### Frontend (TypeScript/Svelte)

- **Image Upload Utility** (`src/lib/utils/imageUpload.ts`):
  - Handles canvas-based image scaling
  - Converts images to base64
  - Communicates with Tauri backend

- **WYSIWYG Editor** (`src/lib/components/WysiwygEditor.svelte`):
  - TipTap Image extension integrated
  - Drop and paste handlers
  - Styled images with hover effects

- **Markdown Editor** (`src/lib/components/MarkdownEditor.svelte`):
  - CodeMirror DOM event handlers for drop/paste
  - Auto-inserts markdown image syntax

- **Markdown Preview** (`src/lib/components/MarkdownPreview.svelte`):
  - Renders markdown images with styling
  - Responsive image sizing

### Backend (Rust/Tauri)

- Asset system already existed in `src-tauri/src/database.rs`
- Commands registered in `src-tauri/src/lib.rs`:
  - `save_image_asset`: Saves base64 image to disk
  - `save_image_bytes`: Saves raw bytes
  - `delete_asset`: Removes asset file
  - `list_assets`: Lists all assets

## Usage

1. **To add an image by dragging:**
   - Simply drag an image file from your file manager into the editor

2. **To add an image by pasting:**
   - Copy an image to clipboard
   - Click in the editor
   - Press Ctrl+V (or Cmd+V on Mac)

3. **Image appearance:**
   - Images are displayed at consistent size (max 800x600)
   - Hover over images to see a subtle zoom effect
   - Images are centered with rounded corners and shadows

## Technical Notes

- Image format: PNG (converted during scaling)
- Storage location: `~/.local/share/com.jfnotes.app/.assets/` on Linux
- Asset protocol: `asset://localhost/[path]`
- Max dimensions: 800x600 pixels (maintains aspect ratio)
