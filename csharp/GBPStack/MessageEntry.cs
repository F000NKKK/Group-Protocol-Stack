namespace GBPStack;

/// <summary>One entry in <see cref="MessageHistory"/>.</summary>
public sealed record MessageEntry(uint SenderId, ulong MessageId, string Text);
