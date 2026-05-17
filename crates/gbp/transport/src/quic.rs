//! QUIC transport for the Group Protocol Stack (quinn-backed).
//!
//! Wire format is identical to the TCP adapter: `u32-LE length || CBOR bytes`.
//! Upper protocol layers are transport-agnostic.
//!
//! # Server
//! ```no_run
//! use gbp_transport::quic::{make_server_endpoint, QuicStream};
//!
//! # async fn run(cert_der: &[u8], key_der: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let endpoint = make_server_endpoint("0.0.0.0:4433".parse()?, cert_der, key_der)?;
//! while let Some(incoming) = endpoint.accept().await {
//!     tokio::spawn(async move {
//!         let conn = incoming.await.unwrap();
//!         let mut stream = QuicStream::accept(&conn).await.unwrap();
//!         let frame = stream.read_frame().await.unwrap();
//!         // … handle frame …
//!     });
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Client
//! ```no_run
//! use gbp_transport::quic::{make_client_endpoint, QuicStream};
//!
//! # async fn run(ca_cert: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let endpoint = make_client_endpoint("0.0.0.0:0".parse()?, Some(ca_cert))?;
//! let conn = endpoint.connect("127.0.0.1:4433".parse()?, "localhost")?.await?;
//! let mut stream = QuicStream::open(&conn).await?;
//! // stream.write_frame(&frame).await?;
//! # Ok(())
//! # }
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};

use gbp::GbpFrame;

use crate::{MAX_FRAME, WireError};

// ── TLS helpers ──────────────────────────────────────────────────────────────

/// Builds a [`ServerConfig`] from DER-encoded certificate and private key.
///
/// The key must be PKCS#8-encoded (`BEGIN PRIVATE KEY`). Use `rcgen` or
/// openssl to generate suitable material for testing.
pub fn make_server_config(cert_der: &[u8], key_der: &[u8]) -> Result<ServerConfig, WireError> {
    let cert = CertificateDer::from(cert_der.to_vec());
    let key = PrivateKeyDer::try_from(key_der.to_vec())
        .map_err(|e| WireError::Quic(format!("invalid private key: {e}")))?;
    ServerConfig::with_single_cert(vec![cert], key)
        .map_err(|e| WireError::Quic(format!("tls server config: {e}")))
}

/// Creates a QUIC server [`Endpoint`] bound to `addr`.
pub fn make_server_endpoint(
    addr: SocketAddr,
    cert_der: &[u8],
    key_der: &[u8],
) -> Result<Endpoint, WireError> {
    let config = make_server_config(cert_der, key_der)?;
    Endpoint::server(config, addr).map_err(|e| WireError::Quic(e.to_string()))
}

/// Creates a QUIC client [`Endpoint`] bound to `local_addr`.
///
/// `ca_cert_der` — DER-encoded CA certificate to trust.
/// Pass `None` to skip TLS verification (useful in local tests only).
pub fn make_client_endpoint(
    local_addr: SocketAddr,
    ca_cert_der: Option<&[u8]>,
) -> Result<Endpoint, WireError> {
    let client_cfg = match ca_cert_der {
        Some(ca) => {
            let mut roots = RootCertStore::empty();
            roots
                .add(CertificateDer::from(ca.to_vec()))
                .map_err(|e| WireError::Quic(format!("add CA cert: {e}")))?;
            ClientConfig::with_root_certificates(Arc::new(roots))
                .map_err(|e| WireError::Quic(format!("tls client config: {e}")))?
        }
        None => {
            let crypto = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(SkipServerVerification::new())
                .with_no_client_auth();
            let quic_cfg = quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
                .map_err(|e| WireError::Quic(format!("quic client crypto: {e}")))?;
            ClientConfig::new(Arc::new(quic_cfg))
        }
    };

    let mut endpoint = Endpoint::client(local_addr).map_err(|e| WireError::Quic(e.to_string()))?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

// ── QuicStream ───────────────────────────────────────────────────────────────

/// A bidirectional QUIC stream with the same framing as the TCP adapter.
///
/// Each side can call [`write_frame`](Self::write_frame) /
/// [`read_frame`](Self::read_frame) independently; frames are not interleaved
/// because QUIC guarantees stream-level ordering.
pub struct QuicStream {
    send: SendStream,
    recv: RecvStream,
}

impl QuicStream {
    /// Opens a new outbound bidirectional stream on an existing connection.
    pub async fn open(conn: &Connection) -> Result<Self, WireError> {
        let (send, recv) = conn
            .open_bi()
            .await
            .map_err(|e| WireError::Quic(e.to_string()))?;
        Ok(Self { send, recv })
    }

    /// Accepts the next inbound bidirectional stream from an existing connection.
    pub async fn accept(conn: &Connection) -> Result<Self, WireError> {
        let (send, recv) = conn
            .accept_bi()
            .await
            .map_err(|e| WireError::Quic(e.to_string()))?;
        Ok(Self { send, recv })
    }

    /// Writes a [`GbpFrame`] using `u32-LE length || CBOR bytes` framing.
    pub async fn write_frame(&mut self, frame: &GbpFrame) -> Result<(), WireError> {
        self.write_blob(&frame.to_cbor()).await
    }

    /// Reads a [`GbpFrame`] using `u32-LE length || CBOR bytes` framing.
    pub async fn read_frame(&mut self) -> Result<GbpFrame, WireError> {
        let buf = self.read_blob().await?;
        Ok(GbpFrame::from_cbor(&buf)?)
    }

    /// Writes an opaque length-prefixed blob.
    pub async fn write_blob(&mut self, data: &[u8]) -> Result<(), WireError> {
        if data.len() > MAX_FRAME {
            return Err(WireError::TooLarge {
                size: data.len(),
                max: MAX_FRAME,
            });
        }
        let len = (data.len() as u32).to_le_bytes();
        self.send
            .write_all(&len)
            .await
            .map_err(|e| WireError::Quic(e.to_string()))?;
        self.send
            .write_all(data)
            .await
            .map_err(|e| WireError::Quic(e.to_string()))?;
        Ok(())
    }

    /// Reads an opaque length-prefixed blob.
    pub async fn read_blob(&mut self) -> Result<Vec<u8>, WireError> {
        let mut len_buf = [0u8; 4];
        self.recv
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| WireError::Quic(e.to_string()))?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_FRAME {
            return Err(WireError::TooLarge {
                size: len,
                max: MAX_FRAME,
            });
        }
        let mut buf = vec![0u8; len];
        self.recv
            .read_exact(&mut buf)
            .await
            .map_err(|e| WireError::Quic(e.to_string()))?;
        Ok(buf)
    }

    /// Gracefully closes the send side of this stream.
    pub async fn finish(&mut self) -> Result<(), WireError> {
        self.send
            .finish()
            .map_err(|e| WireError::Quic(e.to_string()))
    }
}

// ── Skip-verify (test only) ──────────────────────────────────────────────────

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use gbp::GbpFrame;
    use gbp_core::StreamType;
    use rcgen::generate_simple_self_signed;

    fn init_crypto() {
        static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        ONCE.get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    fn make_test_cert() -> (Vec<u8>, Vec<u8>) {
        let cert = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let cert_der = cert.cert.der().to_vec();
        let key_der = cert.key_pair.serialize_der();
        (cert_der, key_der)
    }

    fn test_frame() -> GbpFrame {
        GbpFrame::new(
            [0u8; 16],
            1,
            0,
            StreamType::Control,
            0,
            0,
            42,
            b"payload".to_vec(),
        )
    }

    #[tokio::test]
    async fn quic_blob_round_trip() {
        init_crypto();
        let (cert_der, key_der) = make_test_cert();

        let server_ep =
            make_server_endpoint("127.0.0.1:0".parse().unwrap(), &cert_der, &key_der).unwrap();
        let server_addr = server_ep.local_addr().unwrap();

        let client_ep = make_client_endpoint("127.0.0.1:0".parse().unwrap(), None).unwrap();

        let server_task = tokio::spawn(async move {
            let incoming = server_ep.accept().await.unwrap();
            let conn = incoming.await.unwrap();
            let mut stream = QuicStream::accept(&conn).await.unwrap();
            stream.read_blob().await.unwrap()
        });

        let conn = client_ep
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        let mut stream = QuicStream::open(&conn).await.unwrap();
        let payload = b"hello quic";
        stream.write_blob(payload).await.unwrap();
        stream.finish().await.unwrap();

        let received = server_task.await.unwrap();
        assert_eq!(received, payload);
    }

    #[tokio::test]
    async fn quic_frame_round_trip() {
        init_crypto();
        let (cert_der, key_der) = make_test_cert();

        let server_ep =
            make_server_endpoint("127.0.0.1:0".parse().unwrap(), &cert_der, &key_der).unwrap();
        let server_addr = server_ep.local_addr().unwrap();

        let client_ep = make_client_endpoint("127.0.0.1:0".parse().unwrap(), None).unwrap();

        let server_task = tokio::spawn(async move {
            let incoming = server_ep.accept().await.unwrap();
            let conn = incoming.await.unwrap();
            let mut stream = QuicStream::accept(&conn).await.unwrap();
            stream.read_frame().await.unwrap()
        });

        let conn = client_ep
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        let mut stream = QuicStream::open(&conn).await.unwrap();
        let frame = test_frame();
        stream.write_frame(&frame).await.unwrap();
        stream.finish().await.unwrap();

        let received = server_task.await.unwrap();
        assert_eq!(received.sequence_no, frame.sequence_no);
        assert_eq!(received.encrypted_payload, frame.encrypted_payload);
    }

    #[tokio::test]
    async fn quic_multi_blob_round_trip() {
        init_crypto();
        let (cert_der, key_der) = make_test_cert();

        let server_ep =
            make_server_endpoint("127.0.0.1:0".parse().unwrap(), &cert_der, &key_der).unwrap();
        let server_addr = server_ep.local_addr().unwrap();

        let client_ep = make_client_endpoint("127.0.0.1:0".parse().unwrap(), None).unwrap();

        let server_task = tokio::spawn(async move {
            let incoming = server_ep.accept().await.unwrap();
            let conn = incoming.await.unwrap();
            let mut stream = QuicStream::accept(&conn).await.unwrap();
            let a = stream.read_blob().await.unwrap();
            let b = stream.read_blob().await.unwrap();
            (a, b)
        });

        let conn = client_ep
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        let mut stream = QuicStream::open(&conn).await.unwrap();
        stream.write_blob(b"first").await.unwrap();
        stream.write_blob(b"second").await.unwrap();
        stream.finish().await.unwrap();

        let (a, b) = server_task.await.unwrap();
        assert_eq!(a, b"first");
        assert_eq!(b, b"second");
    }

    #[tokio::test]
    async fn quic_too_large_rejected_on_write() {
        init_crypto();
        let (cert_der, key_der) = make_test_cert();
        let server_ep =
            make_server_endpoint("127.0.0.1:0".parse().unwrap(), &cert_der, &key_der).unwrap();
        let server_addr = server_ep.local_addr().unwrap();
        let client_ep = make_client_endpoint("127.0.0.1:0".parse().unwrap(), None).unwrap();

        // server just needs to exist so client can connect
        tokio::spawn(async move {
            if let Some(inc) = server_ep.accept().await {
                let _ = inc.await;
            }
        });

        let conn = client_ep
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        let mut stream = QuicStream::open(&conn).await.unwrap();
        let oversized = vec![0u8; MAX_FRAME + 1];
        assert!(matches!(
            stream.write_blob(&oversized).await,
            Err(WireError::TooLarge { .. })
        ));
    }
}
