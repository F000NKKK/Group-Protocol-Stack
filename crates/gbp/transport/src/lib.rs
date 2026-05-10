//! TCP transport adapter for the Group Protocol Stack.
//!
//! Each on-the-wire message is framed as `u32-LE length || bytes`. This is a
//! pragmatic stand-in for a QUIC stream; switching to a real QUIC binding
//! later only requires replacing this crate and the same upper layers
//! continue to work.

#![deny(missing_docs)]

use gbp::{CodecError, GbpFrame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Maximum size of a single on-the-wire message (1 MiB).
pub const MAX_FRAME: usize = 1 << 20;

/// Errors raised by the transport layer.
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    /// Underlying I/O error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// GBP base-layer codec error.
    #[error("codec: {0}")]
    Codec(#[from] CodecError),
    /// Message exceeds [`MAX_FRAME`].
    #[error("frame too large: {size} bytes (max {max})")]
    TooLarge {
        /// Observed size.
        size: usize,
        /// Configured limit.
        max: usize,
    },
}

impl WireError {
    fn too_large(n: usize) -> Self {
        Self::TooLarge {
            size: n,
            max: MAX_FRAME,
        }
    }
}

/// Writes a [`GbpFrame`] using `CBOR + length-prefix` framing.
pub async fn write_frame(stream: &mut TcpStream, frame: &GbpFrame) -> Result<(), WireError> {
    let bytes = frame.to_cbor();
    write_blob(stream, &bytes).await
}

/// Reads a [`GbpFrame`] using `length-prefix + CBOR` framing.
pub async fn read_frame(stream: &mut TcpStream) -> Result<GbpFrame, WireError> {
    let buf = read_blob(stream).await?;
    Ok(GbpFrame::from_cbor(&buf)?)
}

/// Writes an opaque length-prefixed blob (e.g. a serialised MLS Welcome or
/// KeyPackage).
pub async fn write_blob(stream: &mut TcpStream, data: &[u8]) -> Result<(), WireError> {
    if data.len() > MAX_FRAME {
        return Err(WireError::too_large(data.len()));
    }
    let len = (data.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(data).await?;
    stream.flush().await?;
    Ok(())
}

/// Reads an opaque length-prefixed blob.
pub async fn read_blob(stream: &mut TcpStream) -> Result<Vec<u8>, WireError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(WireError::too_large(len));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}
