namespace GBPStack;

/// <summary>One frame held by <see cref="JitterBuffer"/>.</summary>
public sealed record AudioFrame(uint MediaSourceId, uint RtpSequence, byte[] Plaintext);
