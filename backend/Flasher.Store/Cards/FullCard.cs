namespace Flasher.Store.Cards;

public record FullCard
{
    public required string Id { get; init; }
    public required string Prompt { get; init; }
    public required string Solution { get; init; }
    public required State State { get; init; }
    public required DateTime ChangeTime { get; init; }
    public required DateTime NextTime { get; init; }
    public required bool Disabled { get; init; }
}
