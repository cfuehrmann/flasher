namespace Flasher.Host.Handlers.Authentication;

public sealed record User
{
    public required string Name { get; init; }
}
