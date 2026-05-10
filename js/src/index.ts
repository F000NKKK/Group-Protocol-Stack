/**
 * Node.js bindings for the Group Protocol Stack.
 *
 * @packageDocumentation
 */

export { lastError, version } from "./native";
export { MlsContext } from "./mls";
export {
    ControlOpcode,
    GroupNode,
    NodeEvent,
    NodeState,
    OutboundFrame,
    StreamType,
} from "./node";
export { GtpAcceptResult, GtpClient } from "./gtp";
export { GapAcceptResult, GapClient } from "./gap";
export { GspAcceptResult, GspClient, SignalType } from "./gsp";

// GTP utilities — message log & resync watermark.
export { MessageEntry, MessageHistory, Watermark } from "./history";

// GAP utility — bounded reorder buffer.
export {
    AudioFrame,
    JitterBuffer,
    JitterPushOutcome,
    JitterPushResult,
} from "./jitter";

// GSP utilities — roles & capability negotiation.
export { Permissions, RoleError, RoleRegistry, RoleSpec } from "./roles";
export { CapabilitiesNegotiator } from "./capabilities";
