namespace GBPStack;

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
