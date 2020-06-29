using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.Extensions.Options;

using Flasher.Store.Authentication;

namespace Flasher.Store.FileStore.Authentication
{
    public class AuthenticationStore : IAuthenticationStore
    {
        private readonly IDictionary<string, string> _users;

        public AuthenticationStore(IOptionsMonitor<FileStoreOptions> options)
        {
            if (options.CurrentValue.Directory == null)
                throw new Exception("Missing configuration 'FileStore:Directory'");

            var json = File.ReadAllText(Path.Combine(options.CurrentValue.Directory, "users.json"));

            _users = JsonSerializer.Deserialize<IDictionary<string, string>>(json);
        }

        public Task<string?> GetPasswordHash(string userName)
        {
            return Task.FromResult(_users.TryGetValue(userName, out string? result) ? result : null);
        }
    }
}