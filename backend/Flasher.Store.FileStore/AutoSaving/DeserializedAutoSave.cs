namespace Flasher.Store.FileStore.AutoSaving;

public sealed record DeserializedAutoSave
{
    public string? Id { get; init; }
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
}
