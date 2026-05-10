//! GBP-layer group node: framing, AEAD, replay window, control plane and
//! FSM.
//!
//! Sub-protocol logic (GTP, GAP, GSP) lives in their own crates; each builds
//! on top of [`GroupNode`] and the [`Sealer`] trait, which acts as the AEAD
//! transport boundary between the protocol layer and the cryptographic
//! provider (`gbp-mls` in the default configuration).

#![deny(missing_docs)]

pub mod node;

pub use node::{DeliveredPayload, Event, GroupNode, NodeError, OutboundFrame, Sealer};
