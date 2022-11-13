using Flasher.Store.AutoSaving;

namespace Flasher.Host.Model;

public sealed record LoginResponse
{
    public required string JsonWebToken { get; init; }
    public AutoSave? AutoSave { get; init; }
}
