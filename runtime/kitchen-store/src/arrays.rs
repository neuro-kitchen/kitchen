//! Arrays in a stored Zarr folder (v2 or v3, sharded or not), read by region: what a viewer asks
//! for a tile of an image or a window of a signal, without the whole result leaving the disk.
//!
//! Elements come out little-endian, in the array's C order, as the data chunks of the runtime API
//! carry them (`u8`, `i16`, `f32`, …).

use std::path::Path;
use std::sync::Arc;

use zarrs::array::data_type::{
    BoolDataType, Float16DataType, Float32DataType, Float64DataType, Int8DataType, Int16DataType, Int32DataType, Int64DataType, UInt8DataType,
    UInt16DataType, UInt32DataType, UInt64DataType,
};
use zarrs::array::{Array, ArrayBytes};
use zarrs::storage::ReadableStorageTraits;

use crate::{ArtifactError, ArtifactResult};

/// An array's shape, element type and chunk shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayInfo {
    /// The element type (`u8`, `u16`, `i32`, `f32`, `f64`, `bool`, …).
    pub dtype: String,
    /// Elements along each dimension.
    pub shape: Vec<u64>,
    /// The first chunk's shape (the size a reader asks for to read whole chunks).
    pub chunks: Vec<u64>,
}

fn open(store: &Path, array: &str) -> ArtifactResult<Array<dyn ReadableStorageTraits>> {
    let err = |e: &dyn std::fmt::Display| ArtifactError::Invalid(format!("{} {array:?}: {e}", store.display()));
    let storage: Arc<dyn ReadableStorageTraits> = Arc::new(zarrs::filesystem::FilesystemStore::new(store).map_err(|e| err(&e))?);
    let path = format!("/{}", array.trim_matches('/'));
    Array::open(storage, &path).map_err(|e| err(&e))
}

fn dtype_of<T: ?Sized>(array: &Array<T>) -> Option<&'static str> {
    let t = array.data_type();
    Some(if t.is::<UInt8DataType>() {
        "u8"
    } else if t.is::<UInt16DataType>() {
        "u16"
    } else if t.is::<UInt32DataType>() {
        "u32"
    } else if t.is::<UInt64DataType>() {
        "u64"
    } else if t.is::<Int8DataType>() {
        "i8"
    } else if t.is::<Int16DataType>() {
        "i16"
    } else if t.is::<Int32DataType>() {
        "i32"
    } else if t.is::<Int64DataType>() {
        "i64"
    } else if t.is::<Float16DataType>() {
        "f16"
    } else if t.is::<Float32DataType>() {
        "f32"
    } else if t.is::<Float64DataType>() {
        "f64"
    } else if t.is::<BoolDataType>() {
        "bool"
    } else {
        return None;
    })
}

/// The array at `array` (a path inside the store: `0` for an OME-Zarr image's full resolution,
/// `""` for an array at the root) in the Zarr folder `store`.
pub fn array_info(store: &Path, array: &str) -> ArtifactResult<ArrayInfo> {
    let a = open(store, array)?;
    let dtype = dtype_of(&a).ok_or_else(|| ArtifactError::Invalid(format!("{array:?}: an element type the runtime does not carry")))?;
    let zeros = vec![0; a.dimensionality()];
    let chunks = a.chunk_shape(&zeros).map_err(|e| ArtifactError::Invalid(e.to_string()))?.iter().map(|c| c.get()).collect();
    Ok(ArrayInfo { dtype: dtype.into(), shape: a.shape().to_vec(), chunks })
}

/// The region of `size` elements from `start` of the array at `array`, little-endian, in C order,
/// with its element type; at most `max_bytes`.
pub fn read_region(store: &Path, array: &str, start: &[u64], size: &[u64], max_bytes: usize) -> ArtifactResult<(String, Vec<u8>)> {
    let a = open(store, array)?;
    let dtype = dtype_of(&a).ok_or_else(|| ArtifactError::Invalid(format!("{array:?}: an element type the runtime does not carry")))?;
    let shape = a.shape();
    if start.len() != shape.len() || size.len() != shape.len() {
        return Err(ArtifactError::Invalid(format!("the array has {} dimensions; the region has {} and {}", shape.len(), start.len(), size.len())));
    }
    for (d, ((&s, &n), &len)) in start.iter().zip(size).zip(shape).enumerate() {
        if s.checked_add(n).is_none_or(|end| end > len) {
            return Err(ArtifactError::Invalid(format!("dimension {d}: {s}..{} is outside 0..{len}", s.saturating_add(n))));
        }
    }
    let element = a.data_type().fixed_size().unwrap_or(1);
    let bytes = size.iter().try_fold(element as u64, |acc, &n| acc.checked_mul(n)).unwrap_or(u64::MAX);
    if bytes > max_bytes as u64 {
        return Err(ArtifactError::Invalid(format!("a region of {bytes} bytes, more than {max_bytes}: ask for smaller regions (whole chunks are {:?})", array_info(store, array)?.chunks)));
    }
    let subset: Vec<std::ops::Range<u64>> = start.iter().zip(size).map(|(&s, &n)| s..s + n).collect();
    let read = |e: &dyn std::fmt::Display| ArtifactError::Invalid(format!("{} {array:?}: {e}", store.display()));
    let mut data = a.retrieve_array_subset::<ArrayBytes<'static>>(&subset).map_err(|e| read(&e))?.into_fixed().map_err(|e| read(&e))?.into_owned();
    if cfg!(target_endian = "big") && element > 1 {
        for v in data.chunks_exact_mut(element) {
            v.reverse();
        }
    }
    Ok((dtype.into(), data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zarrs::array::ArrayBuilder;
    use zarrs::storage::ReadableWritableListableStorage;

    /// A 6×8 `u16` array in 4×4 chunks, element (y, x) = 100·y + x, at `labels/0`.
    pub(crate) fn write_example(dir: &Path) {
        let store: ReadableWritableListableStorage = Arc::new(zarrs::filesystem::FilesystemStore::new(dir).unwrap());
        zarrs::group::GroupBuilder::new().build(store.clone(), "/").unwrap().store_metadata().unwrap();
        let array = ArrayBuilder::new(vec![6, 8], vec![4, 4], zarrs::array::data_type::uint16(), 0u16).build(store, "/labels/0").unwrap();
        array.store_metadata().unwrap();
        let values: Vec<u16> = (0..6u16).flat_map(|y| (0..8u16).map(move |x| 100 * y + x)).collect();
        array.store_array_subset(&array.subset_all(), values).unwrap();
    }

    #[test]
    fn a_region_reads_across_chunks() {
        let dir = std::env::temp_dir().join(format!("kitchen-store-arrays-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_example(&dir);
        let info = array_info(&dir, "labels/0").unwrap();
        assert_eq!(info, ArrayInfo { dtype: "u16".into(), shape: vec![6, 8], chunks: vec![4, 4] });

        // Rows 3–4, columns 2–5: across four chunks
        let (dtype, bytes) = read_region(&dir, "labels/0", &[3, 2], &[2, 4], 1 << 20).unwrap();
        assert_eq!(dtype, "u16");
        let values: Vec<u16> = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        assert_eq!(values, [302, 303, 304, 305, 402, 403, 404, 405]);

        // Outside the array, the wrong rank, too large: refused with why
        assert!(read_region(&dir, "labels/0", &[5, 0], &[2, 1], 1 << 20).unwrap_err().to_string().contains("outside"));
        assert!(read_region(&dir, "labels/0", &[0], &[1], 1 << 20).is_err());
        assert!(read_region(&dir, "labels/0", &[0, 0], &[6, 8], 10).unwrap_err().to_string().contains("smaller regions"));
        assert!(array_info(&dir, "missing").is_err());
    }
}
