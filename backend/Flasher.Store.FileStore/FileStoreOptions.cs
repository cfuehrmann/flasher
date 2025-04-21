using System.ComponentModel.DataAnnotations;

namespace Flasher.Store.FileStore;

public sealed record FileStoreOptions
{
    [Required]
    public required string Directory { get; set; }
}
