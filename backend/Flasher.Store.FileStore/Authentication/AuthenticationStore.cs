using System;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;

using Flasher.Store.Authentication;
using Flasher.Store.Exceptions;

using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.Authentication;

public class AuthenticationStore : IAuthenticationStore
{
    private readonly JsonSerializerContext _jsonContext;
    private readonly string _directory;

    public AuthenticationStore(IOptionsMonitor<FileStoreOptions> options, IFileStoreJsonContextProvider jsonContextProvider)
    {
        _directory = options.CurrentValue.Directory ?? throw new StoreConfigurationException("Missing option 'FileStore:Directory'!");
        _jsonContext = jsonContextProvider.Instance;
    }

    public Task<string?> GetPasswordHash(string userName)
    {
        string path = Path.Combine(_directory, userName, "profile.json");

        if (!File.Exists(path))
        {
            return Task.FromResult<string?>(null);
        }

        string json = File.ReadAllText(path);

        var profile = JsonSerializer.Deserialize(json, typeof(Profile), _jsonContext) as Profile
            ?? throw new InvalidOperationException("The profile file is invalid!");

        return Task.FromResult(profile.PasswordHash);
    }
}
