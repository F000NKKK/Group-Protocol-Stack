using System;
using System.Collections.Generic;
using System.Linq;

namespace GBPStack;

/// <summary>One entry in <see cref="MessageHistory"/>.</summary>
public sealed record MessageEntry(uint SenderId, ulong MessageId, string Text);

/// <summary>
/// Bounded ring-buffer of recent GTP messages. Used for serving resync
/// requests from re-joining peers.
/// </summary>
public sealed class MessageHistory
{
    private readonly int _capacity;
    private readonly LinkedList<MessageEntry> _buffer = new();

    public MessageHistory(int capacity)
    {
        if (capacity <= 0) throw new ArgumentOutOfRangeException(nameof(capacity));
        _capacity = capacity;
    }

    /// <summary>Number of messages currently buffered.</summary>
    public int Count => _buffer.Count;

    /// <summary>Records a message. Returns <c>true</c> if newly added.</summary>
    public bool Push(MessageEntry entry)
    {
        if (Contains(entry.SenderId, entry.MessageId)) return false;
        if (_buffer.Count == _capacity) _buffer.RemoveFirst();
        _buffer.AddLast(entry);
        return true;
    }

    /// <summary>Returns <c>true</c> if <c>(senderId, messageId)</c> is present.</summary>
    public bool Contains(uint senderId, ulong messageId) =>
        _buffer.Any(e => e.SenderId == senderId && e.MessageId == messageId);

    /// <summary>Returns every message produced after the given watermark.</summary>
    public IEnumerable<MessageEntry> Since(Watermark watermark)
    {
        foreach (var m in _buffer)
        {
            var hw = watermark.LastSeen(m.SenderId);
            if (hw is null || m.MessageId > hw.Value) yield return m;
        }
    }

    /// <summary>Returns messages from a single sender newer than <paramref name="sinceMessageId"/>.</summary>
    public IEnumerable<MessageEntry> SinceForSender(uint senderId, ulong sinceMessageId) =>
        _buffer.Where(m => m.SenderId == senderId && m.MessageId > sinceMessageId);

    /// <summary>Drops every message in the buffer.</summary>
    public void Clear() => _buffer.Clear();
}

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
