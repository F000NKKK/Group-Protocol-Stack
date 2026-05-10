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

/// Categorises an MLS message processed via
/// [`MlsContext::process_message`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessedKind {
    /// A Commit message was applied to the group; epoch advanced.
    Commit,
    /// An Application message was decrypted (not used by this stack — GBP
    /// carries application data outside MLS application messages).
    Application,
    /// A Proposal-only message was staged.
    Proposal,
    /// An external message that did not advance the group.
    External,
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
    /// Staged commit produced by [`MlsContext::process_message`] but not
    /// yet merged. Held until [`MlsContext::finalize_pending_commit`] (on
    /// EXECUTE_TRANSITION) so that the local epoch only advances together
    /// with the rest of the group, never earlier — otherwise this side's
    /// READY frame would be sealed under an epoch the coordinator can't
    /// open.
    pub pending_staged: Option<StagedCommit>,
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
                pending_staged: None,
            },
            kp_bundle,
        ))
    }

    /// Result of [`MlsContext::invite_full`]: the Commit message that
    /// existing members must apply via [`MlsContext::process_message`],
    /// plus the Welcome that the new joiner must apply via
    /// [`MlsContext::accept_welcome`].
    ///
    /// RFC 9420 §11/§12.4 — Welcome is for the joiner only; existing members
    /// MUST receive the Commit to advance their epoch.
    ///
    /// IMPORTANT: this call **does not** merge the pending commit. The
    /// caller MUST call [`MlsContext::finalize_pending_commit`] only after
    /// they are confident the Commit/Welcome have been distributed (e.g.
    /// the GBP coordinator has observed READY quorum). If the distribution
    /// fails, call [`MlsContext::clear_pending_commit`] to roll back.
    pub fn invite_full(
        &mut self,
        key_packages: &[KeyPackage],
    ) -> Result<(Vec<u8>, Vec<u8>), MlsError> {
        let (commit, welcome, _gi) = self
            .group
            .add_members(&self.provider, &self.signer, key_packages)
            .map_err(|e| MlsError::OpenMls(format!("add_members: {e:?}")))?;
        let commit_bytes = commit
            .tls_serialize_detached()
            .map_err(|e| MlsError::OpenMls(format!("commit serialize: {e:?}")))?;
        let welcome_bytes = welcome
            .tls_serialize_detached()
            .map_err(|e| MlsError::OpenMls(format!("welcome serialize: {e:?}")))?;
        Ok((commit_bytes, welcome_bytes))
    }

    /// Backwards-compatible wrapper. Builds the Commit, eagerly merges, and
    /// returns only the Welcome bytes. Kept for callers that distribute the
    /// Commit out-of-band and don't need atomic abort semantics.
    pub fn invite(&mut self, key_packages: &[KeyPackage]) -> Result<Vec<u8>, MlsError> {
        let (_commit, welcome) = self.invite_full(key_packages)?;
        self.finalize_pending_commit()?;
        Ok(welcome)
    }

    /// Removes members identified by their MLS LeafIndex via a Remove commit
    /// and returns the TLS-serialised Commit message that remaining members
    /// must apply via [`MlsContext::process_message`].
    ///
    /// Like [`MlsContext::invite_full`], the caller is responsible for
    /// calling [`MlsContext::finalize_pending_commit`] after successful
    /// distribution, or [`MlsContext::clear_pending_commit`] on failure.
    /// RFC 9420 §12.3.
    pub fn remove_members(&mut self, leaf_indices: &[u32]) -> Result<Vec<u8>, MlsError> {
        // Validate indices against the current group size up front so the
        // caller gets a clear error rather than an opaque openmls failure.
        let group_size = self.group.members().count() as u32;
        for &idx in leaf_indices {
            if idx >= group_size {
                return Err(MlsError::OpenMls(format!(
                    "leaf_index {idx} out of range (group size {group_size})"
                )));
            }
        }
        let leaves: Vec<LeafNodeIndex> =
            leaf_indices.iter().copied().map(LeafNodeIndex::new).collect();
        let (commit, _welcome_opt, _gi) = self
            .group
            .remove_members(&self.provider, &self.signer, &leaves)
            .map_err(|e| MlsError::OpenMls(format!("remove_members: {e:?}")))?;
        commit
            .tls_serialize_detached()
            .map_err(|e| MlsError::OpenMls(format!("commit serialize: {e:?}")))
    }

    /// Merges any pending commit. Handles both:
    /// * a self-issued commit produced by [`MlsContext::invite_full`] /
    ///   [`MlsContext::remove_members`] (merged via `merge_pending_commit`);
    /// * a staged commit deposited by [`MlsContext::process_message`]
    ///   (merged via `merge_staged_commit`, consumed from
    ///   [`MlsContext::pending_staged`]).
    ///
    /// Idempotent: if there is nothing to merge, returns Ok. Called from
    /// the GBP control plane in response to `EXECUTE_TRANSITION`.
    pub fn finalize_pending_commit(&mut self) -> Result<(), MlsError> {
        if let Some(staged) = self.pending_staged.take() {
            self.group
                .merge_staged_commit(&self.provider, staged)
                .map_err(|e| MlsError::OpenMls(format!("merge_staged: {e:?}")))?;
        }
        // merge_pending_commit errors if there's nothing to merge — for
        // members that only received a commit (no self-issued one) that's
        // expected, so swallow the error. Self-issued commits are merged
        // via this path on the coordinator side.
        let _ = self.group.merge_pending_commit(&self.provider);
        Ok(())
    }

    /// Discards any pending commit (self-issued and/or staged) without
    /// applying it. Used on `ABORT_TRANSITION`.
    pub fn clear_pending_commit(&mut self) -> Result<(), MlsError> {
        self.pending_staged = None;
        self.group
            .clear_pending_commit(self.provider.storage())
            .map_err(|e| MlsError::OpenMls(format!("clear: {e:?}")))?;
        Ok(())
    }

    /// Applies a Commit (or staged Proposal) message to the group. Existing
    /// members invoke this after receiving the Commit broadcast embedded in
    /// `PREPARE_TRANSITION` args.
    ///
    /// IMPORTANT: a Commit is staged but **not** merged here. It must be
    /// merged via [`MlsContext::finalize_pending_commit`] in response to the
    /// matching `EXECUTE_TRANSITION`, so that this side's MLS epoch
    /// advances together with the rest of the group — never earlier.
    /// Calling this twice without an intervening finalize/clear discards
    /// the previously staged commit (the second call wins).
    pub fn process_message(&mut self, msg_bytes: &[u8]) -> Result<ProcessedKind, MlsError> {
        let msg_in = MlsMessageIn::tls_deserialize_exact_bytes(msg_bytes)
            .map_err(|e| MlsError::OpenMls(format!("msg parse: {e:?}")))?;
        let protocol_msg = match msg_in.extract() {
            MlsMessageBodyIn::PublicMessage(m) => ProtocolMessage::from(m),
            MlsMessageBodyIn::PrivateMessage(m) => ProtocolMessage::from(m),
            other => {
                return Err(MlsError::OpenMls(format!(
                    "expected protocol message, got {other:?}"
                )));
            }
        };
        let processed = self
            .group
            .process_message(&self.provider, protocol_msg)
            .map_err(|e| MlsError::OpenMls(format!("process: {e:?}")))?;
        match processed.into_content() {
            ProcessedMessageContent::StagedCommitMessage(staged) => {
                self.pending_staged = Some(*staged);
                Ok(ProcessedKind::Commit)
            }
            ProcessedMessageContent::ApplicationMessage(_) => Ok(ProcessedKind::Application),
            ProcessedMessageContent::ProposalMessage(_) => Ok(ProcessedKind::Proposal),
            ProcessedMessageContent::ExternalJoinProposalMessage(_) => Ok(ProcessedKind::External),
        }
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
