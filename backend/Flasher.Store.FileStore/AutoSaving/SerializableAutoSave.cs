namespace Flasher.Store.FileStore.AutoSaving;

public sealed record SerializableAutoSave
{
    public string? Id { get; set; }
    public string? Prompt { get; set; }
    public string? Solution { get; set; }
}
