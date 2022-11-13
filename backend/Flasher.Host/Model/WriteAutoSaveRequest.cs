namespace Flasher.Host.Model;

public sealed record WriteAutoSaveRequest
{
    public required string Id { get; init; }
    public required string Prompt { get; init; }
    public required string Solution { get; init; }
}
