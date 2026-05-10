//! MLS (RFC 9420) integration for the Group Protocol Stack.
//!
//! This crate provides:
//!
//! * [`MlsContext`] — a member-side wrapper around an `openmls 0.8` group
//!   (signing key, credential, provider, current group).
//! * [`StreamLabel`] — labelled exporter constants used to derive AEAD keys
//!   from the MLS exporter (`gbp/control`, `gbp/audio`, `gbp/text`,
//!   `gbp/signal`).
//! * `seal` / `open` — ChaCha20-Poly1305 AEAD with the labelled-exporter key.
//!
//! On every epoch change the old key material is invalidated automatically:
//! the AEAD key is derived on the fly from `MlsGroup::export_secret`, never
//! cached, and the previous epoch's secret becomes unreachable as soon as the
//! group ratchets forward.

#![deny(missing_docs)]

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use gbp_core::StreamType;
use openmls::prelude::tls_codec::Serialize as _;
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;

/// MLS ciphersuite used by the stack: X25519-AES128GCM-SHA256-Ed25519.
pub const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

/// Exporter label that binds the AEAD key to a stream class.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StreamLabel {
    /// `gbp/control` — control plane key.
    Control,
    /// `gbp/audio` — GAP key.
    Audio,
    /// `gbp/text` — GTP key.
    Text,
    /// `gbp/signal` — GSP key.
    Signal,
}

impl StreamLabel {
    /// Returns the stable string used as the `MlsGroup::export_secret` label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Control => "gbp/control",
            Self::Audio => "gbp/audio",
            Self::Text => "gbp/text",
            Self::Signal => "gbp/signal",
        }
    }
}

/// Maps a [`StreamType`] to the corresponding [`StreamLabel`].
pub fn label_for(st: StreamType) -> StreamLabel {
    match st {
        StreamType::Control => StreamLabel::Control,
        StreamType::Audio => StreamLabel::Audio,
        StreamType::Text => StreamLabel::Text,
        StreamType::Signal => StreamLabel::Signal,
    }
}

/// Errors raised by the MLS / AEAD layer.
#[derive(Debug, thiserror::Error)]
pub enum MlsError {
    /// Any error returned by `openmls`, serialised as a string.
    #[error("openmls: {0}")]
    OpenMls(String),
    /// AEAD seal or open failure.
    #[error("aead: {0}")]
    Aead(String),
}

/// MLS context for a single group member.
///
/// Owns the OpenMLS provider, the signing key, the credential and the
/// current `MlsGroup`. Ratcheting forward is performed by [`MlsContext::invite`]
/// and [`MlsContext::accept_welcome`].
pub struct MlsContext {
    /// OpenMLS crypto provider.
    pub provider: OpenMlsRustCrypto,
    /// Signing key pair for this member.
    pub signer: SignatureKeyPair,
    /// Current MLS group.
    pub group: MlsGroup,
    /// Credential with the public signing key.
    pub credential: CredentialWithKey,
    /// Member identity (opaque application-defined bytes).
    pub identity: Vec<u8>,
}

impl MlsContext {
    /// Creates a new context with a single-member group, returning the
    /// context together with a [`KeyPackageBundle`] that other members can
    /// use to invite this one.
    pub fn new_member(identity: &[u8]) -> Result<(Self, KeyPackageBundle), MlsError> {
        let provider = OpenMlsRustCrypto::default();
        let signer = SignatureKeyPair::new(CIPHERSUITE.signature_algorithm())
            .map_err(|e| MlsError::OpenMls(format!("signer: {e:?}")))?;
        signer
            .store(provider.storage())
            .map_err(|e| MlsError::OpenMls(format!("store signer: {e:?}")))?;

        let credential = BasicCredential::new(identity.to_vec());
        let credential_with_key = CredentialWithKey {
            credential: credential.into(),
            signature_key: signer.public().into(),
        };

        let kp_bundle = KeyPackage::builder()
            .build(CIPHERSUITE, &provider, &signer, credential_with_key.clone())
            .map_err(|e| MlsError::OpenMls(format!("kp: {e:?}")))?;

        let cfg = MlsGroupCreateConfig::builder()
            .ciphersuite(CIPHERSUITE)
            .use_ratchet_tree_extension(true)
            .build();
        let group = MlsGroup::new(&provider, &signer, &cfg, credential_with_key.clone())
            .map_err(|e| MlsError::OpenMls(format!("group: {e:?}")))?;

        Ok((
            Self {
                provider,
                signer,
                group,
                credential: credential_with_key,
                identity: identity.to_vec(),
            },
            kp_bundle,
        ))
    }

    /// Adds members via an Add commit and returns the TLS-serialised
    /// `Welcome` message that the invitee must consume with
    /// [`MlsContext::accept_welcome`].
    pub fn invite(&mut self, key_packages: &[KeyPackage]) -> Result<Vec<u8>, MlsError> {
        let (_commit, welcome, _gi) = self
            .group
            .add_members(&self.provider, &self.signer, key_packages)
            .map_err(|e| MlsError::OpenMls(format!("add_members: {e:?}")))?;
        self.group
            .merge_pending_commit(&self.provider)
            .map_err(|e| MlsError::OpenMls(format!("merge: {e:?}")))?;
        welcome
            .tls_serialize_detached()
            .map_err(|e| MlsError::OpenMls(format!("welcome serialize: {e:?}")))
    }

    /// Replaces the local group with the one described by the given
    /// `Welcome` message.
    pub fn accept_welcome(&mut self, welcome_bytes: &[u8]) -> Result<(), MlsError> {
        let msg_in = MlsMessageIn::tls_deserialize_exact_bytes(welcome_bytes)
            .map_err(|e| MlsError::OpenMls(format!("welcome parse: {e:?}")))?;
        let welcome = match msg_in.extract() {
            MlsMessageBodyIn::Welcome(w) => w,
            other => {
                return Err(MlsError::OpenMls(format!(
                    "expected welcome, got {other:?}"
                )));
            }
        };
        let join_cfg = MlsGroupJoinConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();
        let staged = StagedWelcome::new_from_welcome(&self.provider, &join_cfg, welcome, None)
            .map_err(|e| MlsError::OpenMls(format!("staged: {e:?}")))?;
        self.group = staged
            .into_group(&self.provider)
            .map_err(|e| MlsError::OpenMls(format!("into_group: {e:?}")))?;
        Ok(())
    }

    /// Returns the current group epoch.
    pub fn epoch(&self) -> u64 {
        self.group.epoch().as_u64()
    }

    /// Returns the 16-byte group identifier (truncated or zero-padded if the
    /// underlying MLS group_id has a different length).
    pub fn group_id_16(&self) -> [u8; 16] {
        let raw = self.group.group_id().as_slice();
        let mut out = [0u8; 16];
        let n = raw.len().min(16);
        out[..n].copy_from_slice(&raw[..n]);
        out
    }

    /// Exports a 32-byte secret under the given stream label.
    pub fn export_stream_key(&self, label: StreamLabel) -> Result<[u8; 32], MlsError> {
        let secret = self
            .group
            .export_secret(self.provider.crypto(), label.as_str(), &[], 32)
            .map_err(|e| MlsError::OpenMls(format!("export: {e:?}")))?;
        let mut out = [0u8; 32];
        out.copy_from_slice(&secret);
        Ok(out)
    }

    /// Encrypts `plaintext` with ChaCha20-Poly1305 using the stream-labelled
    /// AEAD key and a nonce derived from the per-stream `seq`.
    pub fn seal(
        &self,
        label: StreamLabel,
        seq: u32,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, MlsError> {
        let key = self.export_stream_key(label)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&seq.to_be_bytes());
        cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|e| MlsError::Aead(e.to_string()))
    }

    /// Decrypts `ciphertext` with the same parameters as [`MlsContext::seal`].
    pub fn open(
        &self,
        label: StreamLabel,
        seq: u32,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, MlsError> {
        let key = self.export_stream_key(label)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&seq.to_be_bytes());
        cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext)
            .map_err(|e| MlsError::Aead(e.to_string()))
    }
}
