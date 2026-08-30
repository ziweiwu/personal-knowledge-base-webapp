//! Reading and resizing raster images.
//!
//! Two jobs, both driven by the same fact: a knowledge base collects phone screenshots,
//! and a phone screenshot is a several-megabyte PNG of something the reader will see in a
//! column a few hundred pixels wide.

use std::path::Path;

/// The intrinsic size of an image, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

/// Read an image's dimensions without decoding it.
///
/// Only the header is read, which is what makes this affordable during an index build that
/// touches every file in the folder.
pub fn dimensions(path: &Path) -> Option<Dimensions> {
    let reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    Some(Dimensions { width, height })
}

/// What a variant should be encoded as.
///
/// JPEG is dramatically smaller for the photographic content that dominates a knowledge
/// base — a measured 76x on a real iPhone screenshot — but it cannot carry transparency,
/// so anything that actually uses its alpha channel stays PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantFormat {
    Jpeg,
    Png,
}

impl VariantFormat {
    pub fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

/// Where the alpha byte sits in an RGBA pixel.
const ALPHA: usize = 3;

/// Quality high enough that a screenshot's text stays crisp; the size win comes from the
/// format and the scale, not from encoding it badly.
const JPEG_QUALITY: u8 = 82;

/// Why a resize was not produced. Every variant means: serve the original instead.
#[derive(Debug, PartialEq, Eq)]
pub enum VariantError {
    /// Not a raster image this build can decode.
    Unsupported,
    /// The source is already no wider than the request; scaling up would only add bytes.
    AlreadySmallEnough,
    /// Decoding or encoding failed. The original is still perfectly serveable.
    Failed,
}

/// A re-encoded copy of `bytes`, no wider than `target_width`.
///
/// Returns the encoded image and the format it is in. This decodes, so it belongs on a
/// blocking thread rather than in an async handler.
pub fn variant(bytes: &[u8], target_width: u32) -> Result<(Vec<u8>, VariantFormat), VariantError> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| VariantError::Unsupported)?;
    let (natural_width, _) = reader
        .into_dimensions()
        .map_err(|_| VariantError::Unsupported)?;
    if natural_width <= target_width {
        return Err(VariantError::AlreadySmallEnough);
    }

    let decoded = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| VariantError::Unsupported)?
        .decode()
        .map_err(|_| VariantError::Failed)?;

    // Lanczos3 rather than a cheaper filter: this runs once per image per width and is
    // then cached, and downscaled screenshot text is unreadable with nearest or triangle.
    let height = (decoded.height() as u64 * target_width as u64 / decoded.width() as u64).max(1);
    let resized = decoded.resize(
        target_width,
        height as u32,
        image::imageops::FilterType::Lanczos3,
    );

    let format = if uses_transparency(&resized) {
        VariantFormat::Png
    } else {
        VariantFormat::Jpeg
    };
    encode(&resized, format).map(|encoded| (encoded, format))
}

/// Whether any pixel is actually not opaque.
///
/// The channel's presence is not the question: an iPhone screenshot is RGBA and entirely
/// opaque, and treating it as transparent would forfeit the whole saving.
fn uses_transparency(image: &image::DynamicImage) -> bool {
    use image::GenericImageView;
    if !image.color().has_alpha() {
        return false;
    }
    image
        .pixels()
        .any(|(_, _, pixel)| pixel.0[ALPHA] != u8::MAX)
}

fn encode(image: &image::DynamicImage, format: VariantFormat) -> Result<Vec<u8>, VariantError> {
    let mut out = Vec::new();
    let result = match format {
        VariantFormat::Jpeg => image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut std::io::Cursor::new(&mut out),
            JPEG_QUALITY,
        )
        .encode_image(&image.to_rgb8()),
        VariantFormat::Png => {
            image
                .to_rgba8()
                .write_with_encoder(image::codecs::png::PngEncoder::new(
                    &mut std::io::Cursor::new(&mut out),
                ))
        }
    };
    result.map(|()| out).map_err(|_| VariantError::Failed)
}
