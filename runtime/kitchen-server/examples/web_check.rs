//! The server for kitchen-web's browser test (`runtime/kitchen-web/tests/browser/run.sh`): a runtime
//! whose op `lucem:segment` makes a Zarr folder (a 64×96 `u16` array, element (y, x) = 100·y + x),
//! served with the test page from `web_root`. Prints one JSON line: the page's URL, two pairing
//! codes and an input file.

use std::sync::Arc;

use kitchen_client::{Job, Local, Produced, Runner, Runtime};
use kitchen_contract::{Catalog, Format, OpSpec, PortSpec, Type};
use kitchen_network::auth::Authority;
use kitchen_network::pair::Permission;
use kitchen_server::{Server, ServerOptions};
use zarrs::array::ArrayBuilder;
use zarrs::storage::ReadableWritableListableStorage;

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

#[tokio::main]
async fn main() {
    let web_root = std::env::args().nth(1).expect("the web root");
    let dir = std::env::temp_dir().join(format!("kitchen-web-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut catalog = Catalog::new(["image", "labels"].map(Type::new));
    catalog.add(OpSpec::new("lucem:segment", "Segment").input(PortSpec::new("image", "image")).output(PortSpec::new("labels", "labels"))).unwrap();
    let runtime: Arc<dyn Runtime> = Arc::new(Local::new("lucem", catalog, &dir, Arc::new(Segment), 2).unwrap());
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let codes: Vec<String> = (0..2).map(|_| authority.new_pairing_code(Permission::Control).unwrap().display()).collect();
    let image = dir.join("slide.tif");
    std::fs::write(&image, "pixels").unwrap();
    let options = ServerOptions { web_root: Some(web_root.into()), ..ServerOptions::loopback("web check") };
    let server = Server::bind(options, authority, runtime).unwrap();
    let report = serde_json::json!({ "page": server.http_url("127.0.0.1").unwrap(), "codes": codes, "signal": image });
    println!("{report}");
    server.run().await;
}
