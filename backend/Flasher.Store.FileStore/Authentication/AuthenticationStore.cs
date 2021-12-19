using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using Flasher.Store.Authentication;
using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.Authentication;

public class AuthenticationStore : IAuthenticationStore
{
    private readonly IDictionary<string, string> _users;
    private readonly JsonSerializerContext _jsonContext;

    public AuthenticationStore(IOptionsMonitor<FileStoreOptions> options,
        IFileStoreJsonContextProvider jsonContextProvider)
    {
        if (options.CurrentValue.Directory == null)
            throw new ArgumentException("Missing configuration 'FileStore:Directory'");
        _jsonContext = jsonContextProvider.Instance;

        var json = File.ReadAllText(Path.Combine(options.CurrentValue.Directory, "users.json"));
        var type = typeof(IDictionary<string, string>);
        _users = JsonSerializer.Deserialize(json, type, _jsonContext) as IDictionary<string, string>
            ?? throw new InvalidOperationException("Deserializing the users file returned null!");
    }

    public Task<string?> GetPasswordHash(string userName) =>
         Task.FromResult(_users.TryGetValue(userName, out var result) ? result : null);
}
