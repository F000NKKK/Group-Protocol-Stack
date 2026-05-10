"""Python bindings for the Group Protocol Stack.

This package exposes a high-level Python API on top of the native
``gbp_stack`` shared library:

* :class:`MlsContext` - RFC 9420 MLS context.
* :class:`GroupNode` - GBP-layer group node (the IP-like base).
* :class:`GtpClient`, :class:`GapClient`, :class:`GspClient` - sub-protocol
  clients (text, audio, signalling).
* :class:`MessageHistory`, :class:`Watermark` - GTP resync helpers.
* :class:`JitterBuffer` - GAP reorder buffer.
* :class:`RoleRegistry`, :class:`Permissions`, :class:`CapabilitiesNegotiator`
  - GSP roles and capability negotiation.

See :func:`version` for the underlying library version and the project
README for a worked example.
"""

from ._native import last_error, version
from .capabilities import CapabilitiesNegotiator
from .gap_client import GapAcceptResult, GapClient
from .gbp_node import GroupNode, NodeEvent, NodeState, OutboundFrame, StreamType
from .gsp_client import GspAcceptResult, GspClient, SignalType
from .gtp_client import GtpAcceptResult, GtpClient
from .history import MessageEntry, MessageHistory, Watermark
from .jitter import AudioFrame, JitterBuffer, JitterPushOutcome, JitterPushResult
from .mls_context import MlsContext
from .roles import Permissions, RoleError, RoleRegistry, RoleSpec

__all__ = [
    "AudioFrame",
    "CapabilitiesNegotiator",
    "GapAcceptResult",
    "GapClient",
    "GroupNode",
    "GspAcceptResult",
    "GspClient",
    "GtpAcceptResult",
    "GtpClient",
    "JitterBuffer",
    "JitterPushOutcome",
    "JitterPushResult",
    "MessageEntry",
    "MessageHistory",
    "MlsContext",
    "NodeEvent",
    "NodeState",
    "OutboundFrame",
    "Permissions",
    "RoleError",
    "RoleRegistry",
    "RoleSpec",
    "SignalType",
    "StreamType",
    "Watermark",
    "last_error",
    "version",
]

__version__ = "1.1.2"
