namespace Flasher.Host.Model;

public sealed record UpdateCardRequest
{
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
}
