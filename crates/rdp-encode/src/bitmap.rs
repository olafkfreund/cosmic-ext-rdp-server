//! Raw bitmap pass-through encoder.
//!
//! Provides a no-encoding path for use when H.264/EGFX is not available.
//! Frames pass through unmodified as raw BGRA bitmaps, suitable for
//! ironrdp-server's `BitmapUpdate` delivery.

/// Bitmap "encoder" that passes frames through without modification.
///
/// This is a placeholder for the current ironrdp-server architecture
/// which only supports `DisplayUpdate::Bitmap`. When EGFX support is
/// added to ironrdp-server, the [`GstEncoder`](crate::GstEncoder) can
/// be used instead for H.264 delivery.
pub struct BitmapEncoder {
    width: u32,
    height: u32,
}

impl BitmapEncoder {
    /// Create a new bitmap pass-through encoder.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Frame width.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Frame height.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Update dimensions (e.g. on resolution change).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}
