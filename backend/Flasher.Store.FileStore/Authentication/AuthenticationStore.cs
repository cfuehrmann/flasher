using System.Text.Json;
using System.Text.Json.Serialization;
using Flasher.Store.Authentication;
using Flasher.Store.Exceptions;
using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.Authentication;

public class AuthenticationStore(
    IOptionsMonitor<FileStoreOptions> options,
    IFileStoreJsonContextProvider jsonContextProvider
) : IAuthenticationStore
{
    private readonly JsonSerializerContext _jsonContext = jsonContextProvider.Instance;

    public Task<string?> GetPasswordHash(string userName)
    {
        var directory =
            options.CurrentValue.Directory
            ?? throw new StoreConfigurationException("Missing option 'FileStore:Directory'!");

        string path = Path.Combine(directory, userName, "profile.json");

        if (!File.Exists(path))
        {
            return Task.FromResult<string?>(null);
        }

        string json = File.ReadAllText(path);

        var profile =
            JsonSerializer.Deserialize(json, typeof(Profile), _jsonContext) as Profile
            ?? throw new InvalidOperationException("The profile file is invalid!");

        return Task.FromResult(profile.PasswordHash);
    }
}
