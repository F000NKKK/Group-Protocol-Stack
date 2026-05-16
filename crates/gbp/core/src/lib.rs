//! Core type vocabulary for the Group Broadcast Protocol (GBP) stack.
//!
//! This crate has no external dependencies beyond `core`/`alloc` and is the
//! shared foundation for every other crate in the stack:
//!
//! * [`StreamType`] — the four stream classes defined by GBP.
//! * [`GbpFlags`] — frame delivery flag constants.
//! * [`NodeState`], [`TransitionState`], [`SubprotocolState`] — finite state
//!   machines from the state-machine specification.
//! * [`ControlOpcode`] — the control plane opcode registry.
//! * [`SignalType`] — the GSP signal opcode registry.
//! * [`ErrorClass`] and [`codes`] — the error registry.
//! * Type aliases for [`GroupId`], [`MemberId`], [`Epoch`], [`TransitionId`],
//!   [`StreamId`] and [`SequenceNo`].
//!
//! These types intentionally carry no I/O, no serialization and no crypto so
//! that the higher layers can depend on a stable, lightweight vocabulary.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bounded;
pub mod conformance;
pub mod control;
pub mod errors;
pub mod flags;
pub mod ids;
pub mod signal;
pub mod state;
pub mod stream;

pub use bounded::BoundedSeen;
pub use conformance::ConformanceClass;
pub use control::ControlOpcode;
pub use errors::{ErrorClass, codes};
pub use flags::GbpFlags;
pub use ids::{Epoch, GroupId, MemberId, SequenceNo, StreamId, TransitionId};
pub use signal::SignalType;
pub use state::{NodeState, SubprotocolState, TransitionState, timeouts};
pub use stream::StreamType;
