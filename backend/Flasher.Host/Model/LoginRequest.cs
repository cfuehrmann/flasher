namespace Flasher.Host.Model;

public sealed record LoginRequest
{
#nullable disable warnings
    public string UserName { get; init; }
    public string Password { get; init; }
}
