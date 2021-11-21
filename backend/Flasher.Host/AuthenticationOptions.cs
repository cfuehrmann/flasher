using System;

namespace Flasher.Host;

public sealed record AuthenticationOptions
{
    public TimeSpan TokenLifetime { get; init; }
}