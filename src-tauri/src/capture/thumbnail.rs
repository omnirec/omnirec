//! Thumbnail generation utilities.
//!
//! This module provides functions for scaling captured frames and encoding
//! them as JPEG images for use as thumbnails.

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageBuffer, Rgba};

/// Maximum thumbnail width in pixels.
pub const THUMBNAIL_MAX_WIDTH: u32 = 320;

/// Maximum thumbnail height in pixels.
pub const THUMBNAIL_MAX_HEIGHT: u32 = 180;

/// Maximum region preview width in pixels.
pub const PREVIEW_MAX_WIDTH: u32 = 400;

/// Maximum region preview height in pixels.
pub const PREVIEW_MAX_HEIGHT: u32 = 300;

/// JPEG quality for thumbnails (0-100).
const JPEG_QUALITY: u8 = 75;

/// Convert BGRA frame data to a scaled JPEG thumbnail as base64.
///
/// # Arguments
/// * `data` - BGRA pixel data
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `max_width` - Maximum output width
/// * `max_height` - Maximum output height
///
/// # Returns
/// A tuple of (base64_string, scaled_width, scaled_height)
pub fn bgra_to_jpeg_thumbnail(
    data: &[u8],
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> Result<(String, u32, u32), String> {
    if data.len() < (width * height * 4) as usize {
        return Err(format!(
            "Buffer too small: expected {} bytes, got {}",
            width * height * 4,
            data.len()
        ));
    }

    // Convert BGRA to RGBA (swap B and R channels)
    let mut rgba_data = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        rgba_data.push(chunk[2]); // R (was B)
        rgba_data.push(chunk[1]); // G
        rgba_data.push(chunk[0]); // B (was R)
        rgba_data.push(chunk[3]); // A
    }

    // Create image buffer
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_data)
            .ok_or_else(|| "Failed to create image buffer".to_string())?;

    // Calculate scaled dimensions preserving aspect ratio
    let (scaled_width, scaled_height) = calculate_scaled_dimensions(
        width,
        height,
        max_width,
        max_height,
    );

    // Resize using fast Lanczos3 filter for quality
    let resized = image::imageops::resize(
        &img,
        scaled_width,
        scaled_height,
        image::imageops::FilterType::Triangle, // Fast filter for thumbnails
    );

    // Convert to RGB for JPEG encoding (drop alpha)
    let rgb_img = image::DynamicImage::ImageRgba8(resized).to_rgb8();

    // Encode as JPEG
    let mut jpeg_bytes: Vec<u8> = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut jpeg_bytes,
        JPEG_QUALITY,
    );
    encoder
        .encode_image(&rgb_img)
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    // Encode as base64
    let base64_str = STANDARD.encode(&jpeg_bytes);

    Ok((base64_str, scaled_width, scaled_height))
}

/// Calculate scaled dimensions that fit within max bounds while preserving aspect ratio.
fn calculate_scaled_dimensions(
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (max_width, max_height);
    }

    let width_ratio = max_width as f64 / width as f64;
    let height_ratio = max_height as f64 / height as f64;
    let scale = width_ratio.min(height_ratio).min(1.0); // Don't upscale

    let scaled_width = ((width as f64) * scale).round() as u32;
    let scaled_height = ((height as f64) * scale).round() as u32;

    // Ensure at least 1 pixel in each dimension
    (scaled_width.max(1), scaled_height.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_scaled_dimensions_landscape() {
        // 1920x1080 -> max 320x180
        let (w, h) = calculate_scaled_dimensions(1920, 1080, 320, 180);
        assert_eq!(w, 320);
        assert_eq!(h, 180);
    }

    #[test]
    fn test_calculate_scaled_dimensions_portrait() {
        // 1080x1920 -> max 320x180
        let (w, h) = calculate_scaled_dimensions(1080, 1920, 320, 180);
        assert_eq!(w, 101); // Limited by height
        assert_eq!(h, 180);
    }

    #[test]
    fn test_calculate_scaled_dimensions_no_upscale() {
        // 100x50 -> max 320x180 (should not upscale)
        let (w, h) = calculate_scaled_dimensions(100, 50, 320, 180);
        assert_eq!(w, 100);
        assert_eq!(h, 50);
    }

    #[test]
    fn test_bgra_to_jpeg_thumbnail() {
        // Create a small test image (10x10 solid blue in BGRA)
        let width = 10u32;
        let height = 10u32;
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            data.extend_from_slice(&[255, 0, 0, 255]); // BGRA blue
        }

        let result = bgra_to_jpeg_thumbnail(&data, width, height, 320, 180);
        assert!(result.is_ok());

        let (base64_str, scaled_w, scaled_h) = result.unwrap();
        assert!(!base64_str.is_empty());
        assert_eq!(scaled_w, 10); // No upscaling
        assert_eq!(scaled_h, 10);
    }
}
