namespace Flasher.Host.AOT.Handlers.Authentication;

public sealed record LoginRequest
{
    public required string UserName { get; init; }
    public required string Password { get; init; }
}
