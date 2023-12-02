namespace Flasher.Host.AOT.Handlers.Cards;

public sealed record UpdateCardRequest
{
    public required string Id { get; init; }
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
}
