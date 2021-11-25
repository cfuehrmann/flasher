namespace Flasher.Host.Model;

public sealed record WriteAutoSaveRequest
{
#nullable disable warnings
    public string Id { get; init; }
    public string Prompt { get; init; }
    public string Solution { get; init; }
}
