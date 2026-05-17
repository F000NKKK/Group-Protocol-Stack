using System;
using System.Collections.Generic;
using System.Linq;

namespace GBPStack;

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
