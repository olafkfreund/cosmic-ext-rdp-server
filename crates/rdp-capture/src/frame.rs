/// A rectangular region of damage (changed pixels).
#[derive(Debug, Clone)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl DamageRect {
    #[must_use] 
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a damage rect covering the full frame.
    #[must_use] 
    pub fn full_frame(width: u32, height: u32) -> Self {
        Self::new(0, 0, width, height)
    }

    #[must_use] 
    pub fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// Pixel format of captured frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// BGRA with 8 bits per channel (`PipeWire` `BGRx` with alpha = 0xFF).
    Bgra,
    /// RGBA with 8 bits per channel.
    Rgba,
}

impl PixelFormat {
    #[must_use] 
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bgra | Self::Rgba => 4,
        }
    }
}

/// A single captured video frame.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// Raw pixel data (BGRA or RGBA, top-to-bottom row order).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// Row stride in bytes.
    pub stride: u32,
    /// Frame sequence number (monotonically increasing).
    pub sequence: u64,
    /// Damage regions, if available.
    /// `None` means no damage info (treat as full frame).
    /// Empty vec means no damage (frame identical to previous).
    pub damage: Option<Vec<DamageRect>>,
}

impl CapturedFrame {
    /// Convert `BGRx` data to BGRA by setting alpha to 0xFF.
    ///
    /// `PipeWire` typically delivers `BGRx` format where the 'x' padding byte
    /// is undefined. This ensures the alpha channel is fully opaque.
    pub fn ensure_alpha_opaque(&mut self) {
        if self.format == PixelFormat::Bgra {
            for chunk in self.data.chunks_exact_mut(4) {
                chunk[3] = 0xFF;
            }
        }
    }
}
