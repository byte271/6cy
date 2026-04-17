//! # Steganography Utilities
//!
//! This module provides basic least-significant-bit (LSB) steganography
//! operations for embedding data within image pixel buffers.
//!
//! ## Current Status
//!
//! **This is a proof-of-concept implementation.** It operates on raw RGBA
//! byte buffers, not actual image files. To work with real images, you
//! would need to use an image library (e.g., the `image` crate) to decode
//! and encode PNG/BMP files.
//!
//! ## How LSB Steganography Works
//!
//! The least significant bit of each color channel (R, G, B, A) stores one bit
//! of the payload. This means one byte of payload requires 8 pixels (32 bytes
//! of RGBA data) when using all channels.
//!
//! ## Usage
//!
//! ```ignore
//! use sixcy::stego::{embed_in_rgba, extract_from_rgba};
//!
//! // Embed data
//! let mut rgba = vec![0u8; payload.len() * 8 + 1024]; // extra buffer space
//! embed_in_rgba(&mut rgba, &payload)?;
//!
//! // Extract data
//! let recovered = extract_from_rgba(&rgba)?;
//! ```
//!
//! ## Limitations
//!
//! 1. **Buffer-based only** — this module does not read/write image files
//! 2. **No error correction** — bit errors in transmission corrupt the payload
//! 3. **No encryption** — the payload is embedded in plaintext
//! 4. **Capacity depends on carrier size** — ensure the RGBA buffer is large enough

/// Header size for steganographic carrier.
/// Format: [Magic "6CY-STEG" (8) | Payload Size (8) | Reserved (16)]
pub const STEGO_HEADER_SIZE: usize = 32;

/// Magic bytes to identify a steganographic carrier.
pub const STEGO_MAGIC: &[u8; 8] = b"6CY-STEG";

/// Embeds a payload into a raw RGBA buffer using LSB steganography.
///
/// Each byte of the payload (and header) is spread across 8 color channels.
/// For an RGBA image, this means 2 pixels per payload byte.
///
/// # Arguments
///
/// * `rgba` - Raw RGBA pixel buffer (must be at least `(header + payload) * 8` bytes)
/// * `payload` - The data to embed
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(String)` if the buffer is too small
///
/// # Example
///
/// ```ignore
/// let mut buffer = vec![0u8; 10000];
/// let data = b"secret message";
/// embed_in_rgba(&mut buffer, data).unwrap();
/// ```
pub fn embed_in_rgba(rgba: &mut [u8], payload: &[u8]) -> Result<(), String> {
    let mut header = [0u8; STEGO_HEADER_SIZE];
    header[0..8].copy_from_slice(STEGO_MAGIC);
    header[8..16].copy_from_slice(&(payload.len() as u64).to_le_bytes());

    let total_bits = (STEGO_HEADER_SIZE + payload.len()) * 8;
    if rgba.len() < total_bits {
        return Err(format!(
            "Carrier buffer too small. Need {} bytes, have {}.",
            total_bits,
            rgba.len()
        ));
    }

    let mut bit_idx = 0;

    let mut write_bits = |data: &[u8]| {
        for &byte in data {
            for i in 0..8 {
                let bit = (byte >> i) & 1;
                rgba[bit_idx] = (rgba[bit_idx] & 0xFE) | bit;
                bit_idx += 1;
            }
        }
    };

    write_bits(&header);
    write_bits(payload);

    Ok(())
}

/// Extracts a payload from a raw RGBA buffer.
///
/// Reads the header to determine payload size, then extracts the payload bytes.
///
/// # Arguments
///
/// * `rgba` - Raw RGBA pixel buffer containing steganographic data
///
/// # Returns
///
/// * `Ok(Vec<u8>)` containing the extracted payload
/// * `Err(String)` if the buffer is invalid or not a valid carrier
pub fn extract_from_rgba(rgba: &[u8]) -> Result<Vec<u8>, String> {
    if rgba.len() < STEGO_HEADER_SIZE * 8 {
        return Err("Buffer too small for steganographic header".into());
    }

    let read_byte = |start_bit: usize| -> u8 {
        let mut byte = 0u8;
        for i in 0..8 {
            let bit = rgba[start_bit + i] & 1;
            byte |= bit << i;
        }
        byte
    };

    let mut header = [0u8; STEGO_HEADER_SIZE];
    for i in 0..STEGO_HEADER_SIZE {
        header[i] = read_byte(i * 8);
    }

    if &header[0..8] != STEGO_MAGIC {
        return Err("No valid steganographic carrier detected".into());
    }

    let payload_size = u64::from_le_bytes(header[8..16].try_into().unwrap()) as usize;
    let total_bits = (STEGO_HEADER_SIZE + payload_size) * 8;

    if rgba.len() < total_bits {
        return Err("Carrier buffer appears truncated".into());
    }

    let mut payload = Vec::with_capacity(payload_size);
    for i in 0..payload_size {
        payload.push(read_byte((STEGO_HEADER_SIZE + i) * 8));
    }

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_and_extract_roundtrip() {
        let payload = b"Hello, steganography!";
        let capacity = (STEGO_HEADER_SIZE + payload.len()) * 8;
        let mut buffer = vec![128u8; capacity + 256]; // Extra space

        embed_in_rgba(&mut buffer, payload).unwrap();
        let extracted = extract_from_rgba(&buffer).unwrap();

        assert_eq!(extracted, payload);
    }

    #[test]
    fn test_empty_payload() {
        let payload: &[u8] = &[];
        let mut buffer = vec![0u8; STEGO_HEADER_SIZE * 8 + 100];

        embed_in_rgba(&mut buffer, payload).unwrap();
        let extracted = extract_from_rgba(&buffer).unwrap();

        assert!(extracted.is_empty());
    }

    #[test]
    fn test_invalid_carrier_rejected() {
        let buffer = vec![0u8; 1024];
        let result = extract_from_rgba(&buffer);

        assert!(result.is_err());
    }
}
