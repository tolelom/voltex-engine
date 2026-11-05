//! [`DecodedImage`] — the common result type of the PNG/JPEG decoders.

/// A decoded raster image: tightly packed, top-down RGBA8 pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// `width * height * 4` bytes, row-major, 4 bytes (R, G, B, A) per pixel.
    pub rgba: Vec<u8>,
}

#[cfg(test)]
mod tests {
    #[test]
    #[allow(deprecated)]
    fn deprecated_tuple_decoders_match_named_result() {
        let rgba: Vec<u8> = (0..3 * 2 * 4).map(|i| i as u8).collect();
        let png = crate::encode_rgba8(3, 2, &rgba);
        let img = crate::decode_png(&png).unwrap();
        assert_eq!(crate::parse_png(&png).unwrap(), (img.rgba, img.width, img.height));
        assert_eq!(
            crate::parse_jpg(&png).unwrap_err().to_string(),
            crate::decode_jpg(&png).unwrap_err().to_string()
        );
    }
}
