//! A runtime with one stand-in op (`demo:echo`: copies its `signal` input), served on this
//! computer: WebTransport on UDP, Connect / gRPC / gRPC-Web on TCP, same port. Prints the URLs and
//! a token to call it with, e.g. from Python's grpcio or curl:
//!
//! ```text
//! cargo run -p kitchen-server --example serve -- [folder]
//! curl -H "authorization: Bearer $TOKEN" -H "content-type: application/json" \
//!   -d '{"caller": {"via": "curl"}, "ops": {}}' http://127.0.0.1:$PORT/kitchen.runtime.v1.Runtime/Call
//! ```

use std::sync::Arc;

use kitchen_client::{Job, Local, Produced, Runner, Runtime};
use kitchen_contract::{Catalog, Format, OpSpec, PortSpec, Type};
use kitchen_network::auth::Authority;
use kitchen_network::pair::Permission;
use kitchen_server::{Server, ServerOptions};

struct Echo;

impl Runner for Echo {
    fn run(&self, job: &Job, _: &kitchen_client::Cancel, progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String> {
        let input = job.inputs.values().next().ok_or("no input")?;
        let path = job.out_dir.join("echo.bin");
        std::fs::copy(input, &path).map_err(|e| e.to_string())?;
        progress(1, 1);
        Ok(vec![Produced { name: "copy".into(), path, ty: Type::new("signal"), format: Format::parse("raw").unwrap() }])
    }
}

#[tokio::main]
async fn main() {
    let folder = std::env::args().nth(1).map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("kitchen-serve-demo"));
    std::fs::create_dir_all(&folder).unwrap();
    let mut catalog = Catalog::new([Type::new("signal")]);
    catalog.add(OpSpec::new("demo:echo", "Echo").input(PortSpec::new("signal", "signal")).output(PortSpec::new("copy", "signal"))).unwrap();
    let runtime: Arc<dyn Runtime> = Arc::new(Local::new("demo", catalog, &folder, Arc::new(Echo), 2).unwrap());
    let authority = Arc::new(Authority::open_or_create(&folder.join("authority")).unwrap());
    let name = format!("demo-{}", std::process::id());
    let (_, token) = authority.enroll_token(&name, Permission::Control).unwrap();
    let code = authority.new_pairing_code(Permission::Control).unwrap();
    let server = Server::bind(ServerOptions::loopback("kitchen demo"), authority, runtime).unwrap();
    println!("folder       {}", folder.display());
    println!("webtransport {}", server.url("127.0.0.1").unwrap());
    println!("certificate  {}", server.serve_info().certificate_sha256.iter().map(|b| format!("{b:02x}")).collect::<String>());
    println!("http         {}", server.http_url("127.0.0.1").unwrap().unwrap_or_default());
    println!("token        {token}");
    println!("pairing code {}", code.display());
    server.run().await;
}
