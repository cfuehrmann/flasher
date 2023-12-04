namespace Flasher.Host;

public sealed record CardsOptions
{
    public TimeSpan NewCardWaitingTime { get; set; }
    public double OkMultiplier { get; set; }
    public double FailedMultiplier { get; set; }
    public int PageSize { get; set; }
}
