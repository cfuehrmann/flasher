namespace Flasher.Host.AOT.Handlers.Authentication;

public sealed record AuthenticationOptions
{
    public TimeSpan TokenLifetime { get; set; }
}
