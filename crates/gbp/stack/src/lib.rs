//! Top-level facade for the [Group Protocol Stack].
//!
//! Most users should depend on this single crate; the layered architecture
//! is an implementation detail that is exposed for users who only need a
//! subset of the stack.
//!
//! # Layers
//!
//! ```text
//!   ┌────────────────────────────────────────────────────────┐
//!   │  Layer 4 — orchestration                               │
//!   │      gbp-node  →  gbp-stack  →  gbp-stack-ffi          │
//!   ├────────────────────────────────────────────────────────┤
//!   │  Layer 3 — security & transport                        │
//!   │      gbp-mls  ·  gbp-transport                         │
//!   ├────────────────────────────────────────────────────────┤
//!   │  Layer 2 — sub-protocols on top of GBP                 │
//!   │      gtp-protocol · gap-protocol · gsp-protocol        │
//!   ├────────────────────────────────────────────────────────┤
//!   │  Layer 1 — base protocol                               │
//!   │      gbp-protocol (GbpFrame, ControlMessage, …)        │
//!   ├────────────────────────────────────────────────────────┤
//!   │  Layer 0 — vocabulary                                  │
//!   │      gbp-core (StreamType, Flags, FSM, codes)          │
//!   └────────────────────────────────────────────────────────┘
//! ```
//!
//! [Group Protocol Stack]: https://github.com/F000NKKK/Group-Protocol-Stack

#![deny(missing_docs)]

pub use gap;
pub use gbp;
pub use gbp_core as core;
pub use gbp_mls as mls;
pub use gbp_node as node;
pub use gbp_transport as transport;
pub use gsp;
pub use gtp;

pub use gbp::{ControlMessage, ErrorObject, GbpFrame};

pub use gap::{GapAccept, GapClient, GapError, GapPayload};
pub use gsp::{GspAccept, GspClient, GspError, GspSignal};
pub use gtp::{GtpAccept, GtpClient, GtpError, GtpMessage};

pub use gbp_core::{
    ControlOpcode, ErrorClass, GbpFlags, GroupId, MemberId, NodeState, SignalType, StreamType,
    TransitionState, codes,
};

pub use gbp_mls::{MlsContext, ProcessedKind, StreamLabel, label_for};

pub use gbp_node::{DeliveredPayload, Event, GroupNode, NodeError, OutboundFrame, Sealer};

pub use gbp_transport::{MAX_FRAME, WireError, read_blob, read_frame, write_blob, write_frame};
