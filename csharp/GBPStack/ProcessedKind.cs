namespace GBPStack;

/// <summary>Outcome categories for <see cref="MlsContext.ProcessMessage"/>.</summary>
public enum ProcessedKind
{
    /// <summary>Commit applied; epoch advanced.</summary>
    Commit = 1,
    /// <summary>Application message decrypted (not used by GBP).</summary>
    Application = 2,
    /// <summary>Proposal staged (no immediate epoch change).</summary>
    Proposal = 3,
    /// <summary>External message that did not advance the group.</summary>
    External = 4,
}
