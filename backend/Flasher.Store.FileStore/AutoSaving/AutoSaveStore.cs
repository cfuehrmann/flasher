using System;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.Extensions.Options;

using Flasher.Store.AutoSaving;

namespace Flasher.Store.FileStore.AutoSaving
{
    public class AutoSaveStore : IAutoSaveStore
    {
        private readonly string _directory;

        public AutoSaveStore(IOptionsMonitor<FileStoreOptions> options)
        {
            if (options.CurrentValue.Directory == null)
                throw new Exception("Missing configuration 'FileStore:Directory'");

            _directory = options.CurrentValue.Directory;
        }

        public async Task<AutoSave?> Read(string user)
        {
            string path = GetPath(user);

            if (!File.Exists(path)) return null;

            using var fs = File.OpenRead(path);
            var deserialized = await JsonSerializer.DeserializeAsync<DeserializedAutoSave>(fs);

            if (deserialized.id == null) throw new Exception($"The Id of the auto save is null!");
            if (deserialized.prompt == null) throw new Exception($"The Prompt of the auto save is null!");
            if (deserialized.solution == null) throw new Exception($"The Solution of the auto save is null!");

            return new AutoSave(deserialized.id, deserialized.prompt, deserialized.solution);
        }

        public Task Delete(string user)
        {
            string path = GetPath(user);
            File.Delete(path);
            return Task.CompletedTask;
        }

        public async Task Write(string user, AutoSave autoSave)
        {
            string path = GetPath(user);
            using var fs = File.Create(path, 131072, FileOptions.Asynchronous);
            var jsonOptions = new JsonSerializerOptions { WriteIndented = true };
            await JsonSerializer.SerializeAsync<AutoSave>(fs, autoSave, jsonOptions);
        }

        private string GetPath(string user)
        {
            return Path.Combine(_directory, user, "autoSave.json");
        }
    }
}