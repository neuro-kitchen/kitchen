//! Frames on a WebTransport stream: a 4-byte big-endian length, then the bytes (as
//! [`kitchen_wire::write_frame`] on a blocking stream).

use crate::{ServerError, ServerResult};

/// Reads one frame of at most `max` bytes.
pub(crate) async fn read(recv: &mut wtransport::RecvStream, max: usize) -> ServerResult<Vec<u8>> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await.map_err(ServerError::protocol)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > max {
        return Err(ServerError::TooLarge { len, max });
    }
    let mut body = vec![0u8; len];
    recv.read_exact(&mut body).await.map_err(ServerError::protocol)?;
    Ok(body)
}

/// Writes one frame.
pub(crate) async fn write(send: &mut wtransport::SendStream, bytes: &[u8]) -> ServerResult<()> {
    let mut frame = Vec::with_capacity(4 + bytes.len());
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend_from_slice(bytes);
    send.write_all(&frame).await.map_err(ServerError::protocol)
}

/// Sends a JSON message as a frame.
pub(crate) async fn send_json(send: &mut wtransport::SendStream, message: &impl serde::Serialize) -> ServerResult<()> {
    write(send, &serde_json::to_vec(message).map_err(ServerError::protocol)?).await
}

/// Reads a JSON message from a frame.
pub(crate) async fn next_json<M: for<'de> serde::Deserialize<'de>>(recv: &mut wtransport::RecvStream) -> ServerResult<M> {
    serde_json::from_slice(&read(recv, crate::MAX_HELLO).await?).map_err(ServerError::protocol)
}
