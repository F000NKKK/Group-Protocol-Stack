using System.Collections.Generic;

namespace GBPStack;

/// <summary>Per-sender high-water mark of accepted GTP <c>message_id</c>s.</summary>
public sealed class Watermark
{
    private readonly Dictionary<uint, ulong> _lastSeen = new();

    /// <summary>Records that <paramref name="messageId"/> from <paramref name="senderId"/> has been observed.</summary>
    public void Observe(uint senderId, ulong messageId)
    {
        _lastSeen.TryGetValue(senderId, out var prev);
        if (messageId > prev) _lastSeen[senderId] = messageId;
    }

    /// <summary>Last seen <c>message_id</c> from <paramref name="senderId"/>, or <c>null</c>.</summary>
    public ulong? LastSeen(uint senderId) =>
        _lastSeen.TryGetValue(senderId, out var v) ? v : null;

    /// <summary>Iterates every known sender with its last <c>message_id</c>.</summary>
    public IReadOnlyDictionary<uint, ulong> Snapshot() => _lastSeen;

    /// <summary>Drops every entry.</summary>
    public void Clear() => _lastSeen.Clear();
}
