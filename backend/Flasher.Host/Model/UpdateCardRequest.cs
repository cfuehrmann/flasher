namespace Flasher.Host.Model;

public sealed record UpdateCardRequest
{
#nullable disable warnings
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
}
