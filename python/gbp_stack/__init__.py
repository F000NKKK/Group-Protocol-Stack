"""Python bindings for the Group Protocol Stack.

This package exposes a high-level Python API on top of the native
``gbp_stack`` shared library:

* :class:`MlsContext` Р Р†Р вЂљРІР‚Сњ RFC 9420 MLS context.
* :class:`GroupNode` Р Р†Р вЂљРІР‚Сњ GBP-layer group node (the IP-like base).
* :class:`GtpClient`, :class:`GapClient`, :class:`GspClient` Р Р†Р вЂљРІР‚Сњ sub-protocol
  clients (text, audio, signalling).

See :func:`version` for the underlying library version and the project
README for a worked example.
"""

from .gbp_node import GroupNode, NodeEvent, NodeState, OutboundFrame, StreamType
from .gap_client import GapAcceptResult, GapClient
from .gsp_client import GspAcceptResult, GspClient, SignalType
from .gtp_client import GtpAcceptResult, GtpClient
from .mls_context import MlsContext
from ._native import last_error, version

__all__ = [
    "GapAcceptResult",
    "GapClient",
    "GroupNode",
    "GspAcceptResult",
    "GspClient",
    "GtpAcceptResult",
    "GtpClient",
    "MlsContext",
    "NodeEvent",
    "NodeState",
    "OutboundFrame",
    "SignalType",
    "StreamType",
    "last_error",
    "version",
]

__version__ = "1.0.0-rc3"
