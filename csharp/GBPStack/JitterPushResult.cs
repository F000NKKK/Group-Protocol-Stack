namespace GBPStack;

/// <summary>Result returned by <see cref="JitterBuffer.Push"/>.</summary>
public sealed record JitterPushResult(JitterPushOutcome Outcome, AudioFrame? Evicted = null);
