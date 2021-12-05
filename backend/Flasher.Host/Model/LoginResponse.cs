using Flasher.Store.AutoSaving;

namespace Flasher.Host.Model;

public sealed record LoginResponse
{
    public LoginResponse(string jsonWebToken) => JsonWebToken = jsonWebToken;

    public string JsonWebToken { get; }

    public AutoSave? AutoSave { get; init; }
}
