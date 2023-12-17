namespace Flasher.Host.Handlers.Authentication;

public sealed record AuthenticationOptions
{
    public TimeSpan TokenLifetime { get; set; }
}
