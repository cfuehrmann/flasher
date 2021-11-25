namespace Flasher.Host.Model;

public sealed record CreateCardRequest
{
#nullable disable warnings
    public string Prompt { get; init; }
    public string Solution { get; init; }
}
