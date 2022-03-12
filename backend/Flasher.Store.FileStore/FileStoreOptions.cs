namespace Flasher.Store.FileStore;

public sealed record FileStoreOptions
{
    public string? Directory { get; set; }
}