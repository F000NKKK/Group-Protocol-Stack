namespace GBPStack;

/// <summary>
/// Per-member capability advertisement plus intersection / union queries
/// for negotiating optional features (codecs, extensions, version flags).
/// </summary>
public sealed class CapabilitiesNegotiator
{
    private readonly Dictionary<uint, SortedSet<string>> _advertised = new();

    /// <summary>Records an advertisement (replaces any prior one).</summary>
    public void Advertise(uint memberId, IEnumerable<string> capabilities) =>
        _advertised[memberId] = new SortedSet<string>(capabilities);

    /// <summary>Removes a member's advertisement.</summary>
    public void Forget(uint memberId) => _advertised.Remove(memberId);

    /// <summary>Returns the current advertisement for <paramref name="memberId"/>.</summary>
    public IReadOnlyCollection<string>? CapabilitiesOf(uint memberId) =>
        _advertised.TryGetValue(memberId, out var s) ? s : null;

    /// <summary>True if every advertised member supports <paramref name="capability"/>.</summary>
    public bool GroupSupports(string capability)
    {
        if (_advertised.Count == 0) return false;
        return _advertised.Values.All(s => s.Contains(capability));
    }

    /// <summary>Intersection — capabilities every member advertises (safe-to-use set).</summary>
    public IReadOnlySet<string> Intersection()
    {
        if (_advertised.Count == 0) return new HashSet<string>();
        var iter = _advertised.Values.GetEnumerator();
        iter.MoveNext();
        var acc = new HashSet<string>(iter.Current);
        while (iter.MoveNext()) acc.IntersectWith(iter.Current);
        return acc;
    }

    /// <summary>Union — every capability advertised by any member.</summary>
    public IReadOnlySet<string> Union()
    {
        var acc = new HashSet<string>();
        foreach (var s in _advertised.Values)
            acc.UnionWith(s);
        return acc;
    }

    /// <summary>Members that did not advertise <paramref name="capability"/>.</summary>
    public IReadOnlyList<uint> Missing(string capability) =>
        _advertised
            .Where(kv => !kv.Value.Contains(capability))
            .Select(kv => kv.Key)
            .ToList();

    /// <summary>Number of members that advertised something.</summary>
    public int Count => _advertised.Count;

    /// <summary>
    /// Clears all advertisements. Call on epoch advance for symmetry with
    /// <see cref="GtpClient.Reset"/>, <see cref="GapClient.Reset"/> and
    /// <see cref="GspClient.Reset"/>.
    /// </summary>
    public void ResetForEpoch() => _advertised.Clear();
}
