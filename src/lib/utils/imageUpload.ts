import { invoke } from '@tauri-apps/api/core';

export interface ImageUploadResult {
  id: string;
  uri: string;
  path: string;
}

// Maximum dimensions for images (maintains aspect ratio)
const MAX_WIDTH = 800;
const MAX_HEIGHT = 600;

/**
 * Scales an image to fit within MAX_WIDTH and MAX_HEIGHT while maintaining aspect ratio
 */
async function scaleImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    
    reader.onload = (e) => {
      const img = new Image();
      
      img.onload = () => {
        // Calculate new dimensions
        let width = img.width;
        let height = img.height;
        
        // Scale down if necessary
        if (width > MAX_WIDTH || height > MAX_HEIGHT) {
          const widthRatio = MAX_WIDTH / width;
          const heightRatio = MAX_HEIGHT / height;
          const ratio = Math.min(widthRatio, heightRatio);
          
          width = Math.round(width * ratio);
          height = Math.round(height * ratio);
        }
        
        // Create canvas and draw scaled image
        const canvas = document.createElement('canvas');
        canvas.width = width;
        canvas.height = height;
        
        const ctx = canvas.getContext('2d');
        if (!ctx) {
          reject(new Error('Failed to get canvas context'));
          return;
        }
        
        ctx.drawImage(img, 0, 0, width, height);
        
        // Convert to base64
        const scaledBase64 = canvas.toDataURL('image/png');
        resolve(scaledBase64);
      };
      
      img.onerror = () => reject(new Error('Failed to load image'));
      img.src = e.target?.result as string;
    };
    
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsDataURL(file);
  });
}

/**
 * Uploads an image file to Tauri backend with automatic scaling
 * Returns the asset URI that can be used in img src
 */
export async function uploadImage(file: File): Promise<string> {
  try {
    // Scale the image first
    const scaledBase64 = await scaleImage(file);
    
    // Determine file extension
    const extension = file.type.split('/')[1] || 'png';
    
    // Upload to Tauri backend
    const result = await invoke<ImageUploadResult>('save_image_asset', {
      base64Data: scaledBase64,
      fileExtension: extension
    });
    
    return result.uri;
  } catch (error) {
    console.error('Failed to upload image:', error);
    throw error;
  }
}

/**
 * Uploads an image from clipboard data with automatic scaling
 */
export async function uploadImageFromClipboard(blob: Blob): Promise<string> {
  try {
    // Convert blob to File
    const file = new File([blob], 'pasted-image.png', { type: blob.type });
    return await uploadImage(file);
  } catch (error) {
    console.error('Failed to upload clipboard image:', error);
    throw error;
  }
}

/**
 * Uploads an image from a data URL with automatic scaling
 */
export async function uploadImageFromDataUrl(dataUrl: string): Promise<string> {
  try {
    // Determine file extension from data URL
    const match = dataUrl.match(/data:image\/(\w+);base64,/);
    const extension = match ? match[1] : 'png';
    
    // Upload directly (already base64)
    const result = await invoke<ImageUploadResult>('save_image_asset', {
      base64Data: dataUrl,
      fileExtension: extension
    });
    
    return result.uri;
  } catch (error) {
    console.error('Failed to upload image from data URL:', error);
    throw error;
  }
}
