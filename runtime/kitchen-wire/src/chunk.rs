//! Data chunks (decision 0017): a piece of a data stream (a tile, a window, a part of a large
//! result, a table's record batch) as a small header and its bytes, compressed when it helps.
//!
//! - **Arrays** travel raw: `dtype` and `shape` say how to read the little-endian bytes, `region`
//!   and `level` where they sit (as dspk-stream's `SignalFrame.data`).
//! - **Tables** travel as Arrow IPC record batches, which Polars and Python read without converting;
//!   the bytes are the domain's to make (this crate never parses them).
//! - **Compression** is per chunk: `lz4` (fast, for interactive views) or `zstd` (smaller, for
//!   transfers), both in pure Rust so a page decodes them too; `none` for data that does not shrink.

use std::io::Read;

use crate::WireError;
use crate::pb::{Chunk, Region};

/// How a chunk's bytes are compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// As they are.
    None,
    /// LZ4 (block format, size prepended).
    Lz4,
    /// Zstandard.
    Zstd,
}

impl Compression {
    /// Its name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            Compression::None => "none",
            Compression::Lz4 => "lz4",
            Compression::Zstd => "zstd",
        }
    }

    /// The compression named `name`.
    pub fn parse(name: &str) -> Result<Self, WireError> {
        match name {
            "" | "none" => Ok(Compression::None),
            "lz4" => Ok(Compression::Lz4),
            "zstd" => Ok(Compression::Zstd),
            other => Err(WireError(format!("compression {other:?}: none, lz4 or zstd"))),
        }
    }
}

/// An array's chunk: `data` holds `shape`'s elements of `dtype` (little-endian), at `region` of
/// level `level`, compressed with `compression`.
#[allow(clippy::too_many_arguments)]
pub fn array(stream: &str, index: u64, dtype: &str, shape: &[u64], region: Option<Region>, level: u32, data: &[u8], compression: Compression, last: bool) -> Chunk {
    Chunk {
        stream: stream.into(),
        index,
        encoding: "raw".into(),
        compression: compression.name().into(),
        dtype: dtype.into(),
        shape: shape.to_vec(),
        region,
        level,
        data: compress(data, compression),
        last,
    }
}

/// A table's chunk: `ipc` is one Arrow IPC record batch (stream format).
pub fn table(stream: &str, index: u64, ipc: &[u8], compression: Compression, last: bool) -> Chunk {
    Chunk { stream: stream.into(), index, encoding: "arrow-ipc".into(), compression: compression.name().into(), data: compress(ipc, compression), last, ..Default::default() }
}

/// `data` compressed.
pub fn compress(data: &[u8], compression: Compression) -> Vec<u8> {
    match compression {
        Compression::None => data.to_vec(),
        Compression::Lz4 => lz4_flex::compress_prepend_size(data),
        Compression::Zstd => ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest),
    }
}

/// A chunk's bytes, decompressed; for an array, checked against its dtype and shape.
pub fn bytes(chunk: &Chunk) -> Result<Vec<u8>, WireError> {
    let data = match Compression::parse(&chunk.compression)? {
        Compression::None => chunk.data.clone(),
        Compression::Lz4 => lz4_flex::decompress_size_prepended(&chunk.data).map_err(|e| WireError(e.to_string()))?,
        Compression::Zstd => {
            let mut out = Vec::new();
            ruzstd::decoding::StreamingDecoder::new(chunk.data.as_slice()).map_err(|e| WireError(e.to_string()))?.read_to_end(&mut out).map_err(|e| WireError(e.to_string()))?;
            out
        }
    };
    if chunk.encoding == "raw" {
        let size = element_size(&chunk.dtype)?;
        let want = chunk.shape.iter().product::<u64>() as usize * size;
        if data.len() != want {
            return Err(WireError(format!("chunk {} of {}: {} bytes, its shape {:?} of {} needs {want}", chunk.index, chunk.stream, data.len(), chunk.shape, chunk.dtype)));
        }
    }
    Ok(data)
}

/// The size of one element of `dtype`, in bytes.
pub fn element_size(dtype: &str) -> Result<usize, WireError> {
    Ok(match dtype {
        "u8" | "i8" | "bool" => 1,
        "u16" | "i16" | "f16" => 2,
        "u32" | "i32" | "f32" => 4,
        "u64" | "i64" | "f64" => 8,
        other => return Err(WireError(format!("dtype {other:?}"))),
    })
}
