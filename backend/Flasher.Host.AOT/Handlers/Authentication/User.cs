namespace Flasher.Host.AOT.Handlers.Authentication;

public sealed record User
{
    public required string Name { get; init; }
}
