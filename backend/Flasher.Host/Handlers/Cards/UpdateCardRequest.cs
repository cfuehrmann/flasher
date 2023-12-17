namespace Flasher.Host.Handlers.Cards;

public sealed record UpdateCardRequest
{
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
}
