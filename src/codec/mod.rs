use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodecError {
    #[error("Compression error: {0}")]
    Compression(String),
    #[error("Decompression error: {0}")]
    Decompression(String),
    #[error("Unsupported codec ID: {0}")]
    UnsupportedCodec(u8),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Plugin error: {0}")]
    Plugin(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecId {
    None = 0,
    Zstd = 1,
    Lz4 = 2,
    Custom(u8),
}

impl From<u8> for CodecId {
    fn from(id: u8) -> Self {
        match id {
            0 => CodecId::None,
            1 => CodecId::Zstd,
            2 => CodecId::Lz4,
            other => CodecId::Custom(other),
        }
    }
}

pub trait Codec {
    fn id(&self) -> CodecId;
    fn compress(&self, data: &[u8], level: i32) -> Result<Vec<u8>, CodecError>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError>;
}

pub struct ZstdCodec;
impl Codec for ZstdCodec {
    fn id(&self) -> CodecId { CodecId::Zstd }
    fn compress(&self, data: &[u8], level: i32) -> Result<Vec<u8>, CodecError> {
        zstd::encode_all(data, level).map_err(|e| CodecError::Compression(e.to_string()))
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        zstd::decode_all(data).map_err(|e| CodecError::Decompression(e.to_string()))
    }
}

pub struct Lz4Codec;
impl Codec for Lz4Codec {
    fn id(&self) -> CodecId { CodecId::Lz4 }
    fn compress(&self, data: &[u8], _level: i32) -> Result<Vec<u8>, CodecError> {
        Ok(lz4_flex::compress_prepend_size(data))
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        lz4_flex::decompress_size_prepended(data).map_err(|e| CodecError::Decompression(e.to_string()))
    }
}

pub fn get_codec(id: CodecId) -> Result<Box<dyn Codec>, CodecError> {
    match id {
        CodecId::Zstd => Ok(Box::new(ZstdCodec)),
        CodecId::Lz4 => Ok(Box::new(Lz4Codec)),
        CodecId::Custom(id) => {
            Err(CodecError::Plugin(format!("Codec ID {} requires a plugin. See spec.md for details.", id)))
        }
        _ => Err(CodecError::UnsupportedCodec(0)),
    }
}
