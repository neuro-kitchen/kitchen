//! Joins this program's stdin and stdout to an app's MCP socket, so an MCP client that starts a
//! server program (as most do) reaches the running app: `kitchen-mcp-bridge <socket>`. The
//! app prints its socket's path in Settings ▸ Agents.

#[cfg(unix)]
fn main() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: kitchen-mcp-bridge <socket>");
        std::process::exit(2);
    };
    let stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{path}: {e} (is the app running?)");
            std::process::exit(1);
        }
    };
    let mut to_app = stream.try_clone().expect("socket");
    let mut from_app = stream;
    let up = std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 8192];
        while let Ok(n) = stdin.read(&mut buf) {
            if n == 0 || to_app.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = to_app.shutdown(std::net::Shutdown::Write);
    });
    let mut stdout = std::io::stdout().lock();
    let mut buf = [0u8; 8192];
    while let Ok(n) = from_app.read(&mut buf) {
        if n == 0 || stdout.write_all(&buf[..n]).and_then(|_| stdout.flush()).is_err() {
            break;
        }
    }
    let _ = up.join();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("kitchen-mcp-bridge needs a Unix socket; on this system serve the app over stdio");
    std::process::exit(2);
}
