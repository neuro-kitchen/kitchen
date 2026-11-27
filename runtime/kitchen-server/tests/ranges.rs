//! A result kept as a Zarr folder, read by region over WebTransport (uncompressed, lz4, zstd) as a
//! viewer reads tiles: never the whole result.

use std::collections::BTreeMap;
use std::sync::Arc;

use kitchen_client::{Answer, Caller, Job, Local, Produced, Request, Runner, Runtime, RuntimeError, Source, Target};
use kitchen_contract::{Catalog, Format, OpSpec, PortSpec, RunStatus, Type};
use kitchen_network::auth::Authority;
use kitchen_network::pair::Permission;
use kitchen_server::{Remote, RemoteOptions, Server, ServerOptions};
use zarrs::array::ArrayBuilder;
use zarrs::storage::ReadableWritableListableStorage;

/// `segment` writes labels: a 64×96 `u16` array in 16×16 chunks at `0`, element (y, x) = 100·y + x.
struct Segment;

impl Runner for Segment {
    fn run(&self, job: &Job, _: &kitchen_client::Cancel, _: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String> {
        let dir = job.out_dir.join("labels.zarr");
        let store: ReadableWritableListableStorage = Arc::new(zarrs::filesystem::FilesystemStore::new(&dir).map_err(|e| e.to_string())?);
        zarrs::group::GroupBuilder::new().build(store.clone(), "/").map_err(|e| e.to_string())?.store_metadata().map_err(|e| e.to_string())?;
        let array = ArrayBuilder::new(vec![64, 96], vec![16, 16], zarrs::array::data_type::uint16(), 0u16).build(store, "/0").map_err(|e| e.to_string())?;
        array.store_metadata().map_err(|e| e.to_string())?;
        let values: Vec<u16> = (0..64u16).flat_map(|y| (0..96u16).map(move |x| 100 * y + x)).collect();
        array.store_array_subset(&array.subset_all(), values).map_err(|e| e.to_string())?;
        Ok(vec![Produced { name: "labels".into(), path: dir, ty: Type::new("labels"), format: Format::parse("ome-zarr").unwrap() }])
    }
}

fn u16s(bytes: &[u8]) -> Vec<u16> {
    bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect()
}

#[test]
fn a_zarr_result_is_read_by_region_over_webtransport() {
    let dir = std::env::temp_dir().join(format!("kitchen-server-ranges-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut catalog = Catalog::new(["image", "labels"].map(Type::new));
    catalog.add(OpSpec::new("lucem:segment", "Segment").input(PortSpec::new("image", "image")).output(PortSpec::new("labels", "labels"))).unwrap();
    let local = Arc::new(Local::new("lucem", catalog, &dir, Arc::new(Segment), 2).unwrap());
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let (_, token) = authority.enroll_token("ana", Permission::Control).unwrap();

    let runtime: Arc<dyn Runtime> = local.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async move {
            let server = Server::bind(ServerOptions::loopback("test"), authority, runtime).unwrap();
            tx.send(RemoteOptions { url: server.url("127.0.0.1").unwrap(), certificate_sha256: server.serve_info().certificate_sha256.clone() }).unwrap();
            server.run().await;
        });
    });
    let remote = Remote::connect(&rx.recv().unwrap(), &token).unwrap();
    let me = Caller::new("ana", "viewer");

    // Run segment: its labels kept by reference, a Zarr folder
    let image = dir.join("slide.tif");
    std::fs::write(&image, "pixels").unwrap();
    let filling: BTreeMap<String, Source> = [("image".into(), Source::Path { path: image, ty: Type::new("image"), format: Format::parse("tiff").unwrap() })].into();
    let record = remote.wait(&me, &remote.run(&me, Target::Op { op: "lucem:segment".into(), params: BTreeMap::new() }, filling).unwrap()).unwrap();
    assert_eq!(record.status, RunStatus::Succeeded);
    let labels = record.outputs["labels"].clone();

    // A folder is not read whole
    assert!(matches!(remote.read(&me, &labels), Err(RuntimeError::Invalid(why)) if why.contains("by range")));

    // Its array, then a tile across four chunks, as the in-process runtime answers it
    assert_eq!(remote.array(&me, &labels, "0").unwrap(), ("u16".to_string(), vec![64, 96], vec![16, 16]));
    let (dtype, bytes) = remote.range(&me, &labels, "0", &[14, 30], &[4, 5]).unwrap();
    assert_eq!(dtype, "u16");
    assert_eq!(u16s(&bytes)[..5], [1430, 1431, 1432, 1433, 1434]);
    assert_eq!(u16s(&bytes)[15..], [1730, 1731, 1732, 1733, 1734]);
    assert_eq!(local.range(&me, &labels, "0", &[14, 30], &[4, 5]).unwrap().1, bytes);

    // Compressed on the wire, the same elements out
    for compression in ["lz4", "zstd"] {
        let ask = Request::ResultRange { id: labels.clone(), array: "0".into(), start: vec![0, 0], size: vec![64, 96], compression: compression.into() };
        let Answer::Region { data, compression: how, .. } = remote.call(&me, ask).unwrap() else { panic!("not a region") };
        assert_eq!(how, compression);
        assert_eq!(u16s(&data)[96 * 63 + 95], 6395);
    }

    // Outside the array; a compression nobody knows
    assert!(matches!(remote.range(&me, &labels, "0", &[60, 0], &[8, 1]), Err(RuntimeError::Invalid(why)) if why.contains("outside")));
    let ask = Request::ResultRange { id: labels, array: "0".into(), start: vec![0, 0], size: vec![1, 1], compression: "brotli".into() };
    assert!(matches!(remote.call(&me, ask), Err(RuntimeError::Invalid(_))));
}
