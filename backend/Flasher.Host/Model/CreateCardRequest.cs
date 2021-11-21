namespace Flasher.Host.Model;

public sealed record CreateCardRequest
{
#nullable disable warnings
    public string prompt { get; init; }
    public string solution { get; init; }
}