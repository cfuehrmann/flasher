namespace Flasher.Host.Model;

public sealed record CreateCardRequest
{
    public required string Prompt { get; init; }
    public required string Solution { get; init; }
}
