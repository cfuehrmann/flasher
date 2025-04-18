namespace Flasher.Store.Authentication;

public interface IAuthenticationStore
{
    Task<string?> GetPasswordHash(string userName);
}
