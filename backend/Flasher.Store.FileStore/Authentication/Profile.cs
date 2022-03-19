namespace Flasher.Store.FileStore.Authentication;

public sealed record Profile
{
    public string? UserName { get; set; }
    public string? PasswordHash { get; set; }
}