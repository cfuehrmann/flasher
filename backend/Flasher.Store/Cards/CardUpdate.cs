namespace Flasher.Store.Cards;

public record CardUpdate
{
    public required string Id { get; init; }
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
    public State? State { get; init; }
    public DateTime? ChangeTime { get; init; }
    public DateTime? NextTime { get; init; }
    public bool? Disabled { get; init; }
}
