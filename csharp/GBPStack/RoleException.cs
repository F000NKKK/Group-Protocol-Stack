using System;

namespace GBPStack;

/// <summary>Errors thrown by <see cref="RoleRegistry"/>.</summary>
public sealed class RoleException : Exception
{
    public RoleException(string message) : base(message) { }
}
