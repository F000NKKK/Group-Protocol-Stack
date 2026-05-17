using System;
using System.Text.Json;

namespace GBPStack;

/// <summary>
/// Event surfaced by the GBP layer.
/// <para><c>Kind</c> determines which optional fields are populated:</para>
/// <list type="bullet">
///   <item><term>state_changed</term><description>FromState, ToState.</description></item>
///   <item><term>payload_received</term><description>StreamTypeName, StreamType, StreamId, SequenceNo, Flags, Plaintext.</description></item>
///   <item><term>control</term><description>Sender, Opcode, OpcodeCode, TransitionId, RequestId, Args.</description></item>
///   <item><term>error</term><description>Code, CodeHex, Class, Retryable, Fatal, Reason.</description></item>
///   <item><term>epoch_advanced</term><description>Epoch, TransitionId.</description></item>
///   <item><term>coordinator_election_needed</term><description>No extra fields — the local node should start the coordinator-election handshake (GSP COORDINATOR_CLAIM).</description></item>
///   <item><term>became_coordinator</term><description>No extra fields — this node won the election.</description></item>
///   <item><term>coordinator_claim</term><description>Claimant — member id of the peer that sent COORDINATOR_CLAIM.</description></item>
/// </list>
/// </summary>
public sealed record NodeEvent(
    string Kind,
    string? FromState = null,
    string? ToState = null,
    string? StreamTypeName = null,
    StreamType? StreamType = null,
    uint? StreamId = null,
    uint? SequenceNo = null,
    ushort? Flags = null,
    byte[]? Plaintext = null,
    uint? Sender = null,
    string? Opcode = null,
    ushort? OpcodeCode = null,
    uint? TransitionId = null,
    uint? RequestId = null,
    byte[]? Args = null,
    ushort? Code = null,
    string? CodeHex = null,
    byte? Class = null,
    bool? Retryable = null,
    bool? Fatal = null,
    string? Reason = null,
    ulong? Epoch = null,
    uint? Claimant = null)
{
    internal static NodeEvent FromJson(JsonElement el)
    {
        var kind = el.GetProperty("kind").GetString() ?? "";
        string? S(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.String ? x.GetString() : null;
        uint? U32(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetUInt32() : null;
        ushort? U16(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetUInt16() : null;
        ulong? U64(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetUInt64() : null;
        byte? U8(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetByte() : null;
        bool? B(string n) => el.TryGetProperty(n, out var x) && x.ValueKind is JsonValueKind.True or JsonValueKind.False ? x.GetBoolean() : null;

        byte[]? plaintext = null;
        if (el.TryGetProperty("plaintext_b64", out var pb) && pb.ValueKind == JsonValueKind.String)
            plaintext = Convert.FromBase64String(pb.GetString()!);

        byte[]? args = null;
        if (el.TryGetProperty("args_b64", out var ab) && ab.ValueKind == JsonValueKind.String)
        {
            var s = ab.GetString();
            if (!string.IsNullOrEmpty(s)) args = Convert.FromBase64String(s);
        }

        StreamType? st = null;
        if (el.TryGetProperty("stream_type_code", out var stc) && stc.ValueKind == JsonValueKind.Number)
            st = (StreamType)stc.GetUInt32();

        return new NodeEvent(
            kind,
            FromState: S("from"),
            ToState: S("to"),
            StreamTypeName: S("stream_type"),
            StreamType: st,
            StreamId: U32("stream_id"),
            SequenceNo: U32("sequence_no"),
            Flags: U16("flags"),
            Plaintext: plaintext,
            Sender: U32("from"),
            Opcode: S("opcode"),
            OpcodeCode: U16("opcode_code"),
            TransitionId: U32("transition_id"),
            RequestId: U32("request_id"),
            Args: args,
            Code: U16("code"),
            CodeHex: S("code_hex"),
            Class: U8("class"),
            Retryable: B("retryable"),
            Fatal: B("fatal"),
            Reason: S("reason"),
            Epoch: U64("epoch"),
            Claimant: U32("claimant"));
    }
}
