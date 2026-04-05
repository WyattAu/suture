//! Zstd compression/decompression utilities.
//!
//! Zstd provides an excellent balance of compression ratio and speed.
//! Default compression level is 3, which provides ~2-3x compression on
//! typical project data with minimal CPU overhead.

use std::io::{Read, Write};

/// Default Zstd compression level.
/// Level 3 provides a good balance of speed (~500 MB/s) and ratio (~2.5x).
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

/// Maximum decompressed size to prevent zip bombs.
/// 1 GB is a reasonable safety limit for v0.1.
pub const MAX_DECOMPRESSED_SIZE: usize = 1024 * 1024 * 1024;

/// Compress data using Zstd at the given level.
///
/// Returns the compressed bytes. For very small inputs (< 100 bytes),
/// compression may increase size; callers should check and store uncompressed
/// if that's the case.
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>, super::CasError> {
    let mut encoder = zstd::Encoder::new(Vec::new(), level)
        .map_err(|e| super::CasError::CompressionError(e.to_string()))?;
    encoder
        .write_all(data)
        .map_err(|e| super::CasError::CompressionError(e.to_string()))?;
    let compressed = encoder
        .finish()
        .map_err(|e| super::CasError::CompressionError(e.to_string()))?;

    // If compression didn't help, return original data with a flag
    // (For simplicity, we always use zstd frame format which includes
    // the uncompressed size in the header)
    Ok(compressed)
}

/// Compress data at the default level (3).
#[allow(dead_code)]
pub fn compress_default(data: &[u8]) -> Result<Vec<u8>, super::CasError> {
    compress(data, DEFAULT_COMPRESSION_LEVEL)
}

/// Decompress data using Zstd.
///
/// Validates that the decompressed size does not exceed MAX_DECOMPRESSED_SIZE
/// to prevent zip bomb attacks.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, super::CasError> {
    let mut decoder =
        zstd::Decoder::new(data).map_err(|e| super::CasError::DecompressionError(e.to_string()))?;

    // Use a bounded reader to prevent zip bombs
    let mut output = Vec::with_capacity(data.len() * 2); // Heuristic initial size
    let mut buffer = [0u8; 64 * 1024]; // 64 KB read buffer

    loop {
        let n = decoder
            .read(&mut buffer)
            .map_err(|e| super::CasError::DecompressionError(e.to_string()))?;
        if n == 0 {
            break;
        }
        if output.len() + n > MAX_DECOMPRESSED_SIZE {
            return Err(super::CasError::DecompressionTooLarge {
                max: MAX_DECOMPRESSED_SIZE,
            });
        }
        output.extend_from_slice(&buffer[..n]);
    }

    Ok(output)
}

/// Check if data appears to be Zstd-compressed (magic number check).
///
/// Zstd frames start with 0x28 0xB5 0x2F 0xFD.
pub fn is_zstd_compressed(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0x28 && data[1] == 0xB5 && data[2] == 0x2F && data[3] == 0xFD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"Hello, Suture! This is test data for compression roundtrip.";
        let compressed = compress_default(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compress_larger_data() {
        let original: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let compressed = compress_default(&original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, decompressed);

        // Compressed should be smaller (repetitive data compresses well)
        assert!(compressed.len() < original.len());
    }

    #[test]
    fn test_compress_empty() {
        let original = b"";
        let compressed = compress_default(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_is_zstd_compressed() {
        let data = b"hello";
        assert!(!is_zstd_compressed(data));

        let compressed = compress_default(data).unwrap();
        assert!(is_zstd_compressed(&compressed));
    }

    #[test]
    fn test_decompress_invalid_data() {
        let result = decompress(b"not zstd data at all!");
        assert!(result.is_err());
    }

    #[test]
    fn test_compress_levels() {
        let data = "The quick brown fox jumps over the lazy dog. ".repeat(1000);
        let bytes = data.as_bytes();

        let c1 = compress(bytes, 1).unwrap();
        let c3 = compress(bytes, 3).unwrap();
        let c9 = compress(bytes, 9).unwrap();

        // Higher compression should generally produce smaller output
        assert!(c9.len() <= c3.len());
        assert!(c3.len() <= c1.len());
    }
}
