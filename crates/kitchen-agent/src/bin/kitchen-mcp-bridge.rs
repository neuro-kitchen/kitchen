//! Joins this program's stdin and stdout to an app's MCP socket, so an MCP client that starts a
//! server program (as most do) reaches the running app: `kitchen-mcp-bridge <socket>`. The
//! app prints its socket's path in Settings ▸ Agents.

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: kitchen-mcp-bridge <socket>");
        std::process::exit(2);
    };
    if let Err(e) = kitchen_agent::bridge::bridge_stdio(std::path::Path::new(&path)) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
