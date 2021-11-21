namespace Flasher.Store.FileStore.AutoSaving;

public sealed record DeserializedAutoSave
{
    public string? id { get; init; }
    public string? prompt { get; init; }
    public string? solution { get; init; }
}