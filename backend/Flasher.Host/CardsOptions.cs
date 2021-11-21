using System;

namespace Flasher.Host;

public sealed record CardsOptions
{
    public TimeSpan NewCardWaitingTime { get; init; }
    public double OkMultiplier { get; init; }
    public double FailedMultiplier { get; init; }
    public int PageSize { get; init; }
}