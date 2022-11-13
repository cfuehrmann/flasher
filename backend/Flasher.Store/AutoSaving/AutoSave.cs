namespace Flasher.Store.AutoSaving;

public sealed record AutoSave
{
    public required string Id { get; init; }
    public required string Prompt { get; init; }
    public required string Solution { get; init; }
}
