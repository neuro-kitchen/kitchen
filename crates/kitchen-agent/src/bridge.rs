//! Joins a program's stdin and stdout to an app's MCP socket, so an MCP client that starts a
//! server program (as most do, ACP agents too) reaches the running app. An app serves it from its
//! own executable (`lumen-app --mcp-bridge <socket>`), or `kitchen-mcp-bridge <socket>` does.

use std::path::Path;

/// Copies stdin to the socket at `path` and the socket to stdout until either side closes.
#[cfg(unix)]
pub fn bridge_stdio(path: &Path) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let stream = UnixStream::connect(path).map_err(|e| format!("{}: {e} (is the app running?)", path.display()))?;
    let mut to_app = stream.try_clone().map_err(|e| e.to_string())?;
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
    Ok(())
}

/// Not on this system: the app serves MCP over stdio instead.
#[cfg(not(unix))]
pub fn bridge_stdio(_: &Path) -> Result<(), String> {
    Err("the MCP bridge needs a Unix socket; on this system serve the app over stdio".into())
}
