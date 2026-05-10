using System;
using System.Collections.Generic;

namespace GBPStack;

/// <summary>One frame held by <see cref="JitterBuffer"/>.</summary>
public sealed record AudioFrame(uint MediaSourceId, uint RtpSequence, byte[] Plaintext);

/// <summary>Outcome of <see cref="JitterBuffer.Push"/>.</summary>
public enum JitterPushOutcome
{
    /// <summary>Buffered successfully.</summary>
    Accepted,
    /// <summary>Dropped — older than the next-expected sequence.</summary>
    Late,
    /// <summary>Buffered, evicting the oldest queued frame to make room.</summary>
    Evicted
}

/// <summary>Result returned by <see cref="JitterBuffer.Push"/>.</summary>
public sealed record JitterPushResult(JitterPushOutcome Outcome, AudioFrame? Evicted = null);

/// <summary>
/// Bounded reorder window for GAP audio frames. Hold them briefly so the
/// decoder consumes them in <c>rtp_sequence</c> order, drop late arrivals.
/// </summary>
public sealed class JitterBuffer
{
    private sealed class SourceState
    {
        public readonly LinkedList<AudioFrame> Waiting = new();
        public uint? Next;
    }

    private readonly int _capacityPerSource;
    private readonly Dictionary<uint, SourceState> _sources = new();

    public JitterBuffer(int capacityPerSource)
    {
        if (capacityPerSource <= 0) throw new ArgumentOutOfRangeException(nameof(capacityPerSource));
        _capacityPerSource = capacityPerSource;
    }

    /// <summary>Inserts a frame.</summary>
    public JitterPushResult Push(AudioFrame frame)
    {
        if (!_sources.TryGetValue(frame.MediaSourceId, out var state))
        {
            state = new SourceState();
            _sources[frame.MediaSourceId] = state;
        }
        if (state.Next is { } next && frame.RtpSequence < next)
            return new JitterPushResult(JitterPushOutcome.Late);

        // Already present?
        for (var node = state.Waiting.First; node != null; node = node.Next)
            if (node.Value.RtpSequence == frame.RtpSequence)
                return new JitterPushResult(JitterPushOutcome.Accepted);

        // Insert in sorted order.
        var inserted = false;
        for (var node = state.Waiting.First; node != null; node = node.Next)
        {
            if (node.Value.RtpSequence > frame.RtpSequence)
            {
                state.Waiting.AddBefore(node, frame);
                inserted = true;
                break;
            }
        }
        if (!inserted) state.Waiting.AddLast(frame);

        if (state.Waiting.Count > _capacityPerSource)
        {
            var evicted = state.Waiting.First!.Value;
            state.Waiting.RemoveFirst();
            return new JitterPushResult(JitterPushOutcome.Evicted, evicted);
        }
        return new JitterPushResult(JitterPushOutcome.Accepted);
    }

    /// <summary>Pops the next frame in order, or <c>null</c> if the head is from the future.</summary>
    public AudioFrame? PopInOrder(uint mediaSourceId)
    {
        if (!_sources.TryGetValue(mediaSourceId, out var state)) return null;
        if (state.Waiting.First is null) return null;
        var head = state.Waiting.First.Value;
        if (state.Next is { } next && head.RtpSequence != next) return null;
        state.Waiting.RemoveFirst();
        state.Next = head.RtpSequence + 1;
        return head;
    }

    /// <summary>Pops the next frame regardless of contiguity (skips gaps).</summary>
    public AudioFrame? PopForce(uint mediaSourceId)
    {
        if (!_sources.TryGetValue(mediaSourceId, out var state)) return null;
        if (state.Waiting.First is null) return null;
        var head = state.Waiting.First.Value;
        state.Waiting.RemoveFirst();
        state.Next = head.RtpSequence + 1;
        return head;
    }

    /// <summary>Number of frames buffered for <paramref name="mediaSourceId"/>.</summary>
    public int LengthFor(uint mediaSourceId) =>
        _sources.TryGetValue(mediaSourceId, out var s) ? s.Waiting.Count : 0;

    /// <summary>Drops every queued frame.</summary>
    public void Clear() => _sources.Clear();
}
