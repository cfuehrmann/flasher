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
        private readonly string _path;

        public AutoSaveStore(IOptionsMonitor<FileStoreOptions> options)
        {
            if (options.CurrentValue.Directory == null)
                throw new Exception("Missing configuration 'FileStore:Directory'");

            _path = Path.Combine(options.CurrentValue.Directory, "autoSave.json");
        }

        public async Task<AutoSave?> Read(string user)
        {
            if (!File.Exists(_path)) return null;

            using var fs = File.OpenRead(_path);
            var deserialized = await JsonSerializer.DeserializeAsync<DeserializedAutoSave>(fs);

            if (deserialized.id == null) throw new Exception($"The Id of the auto save is null!");
            if (deserialized.prompt == null) throw new Exception($"The Prompt of the auto save is null!");
            if (deserialized.solution == null) throw new Exception($"The Solution of the auto save is null!");

            return new AutoSave(deserialized.id, deserialized.prompt, deserialized.solution);
        }

        public Task Delete(string user)
        {
            File.Delete(_path);
            return Task.CompletedTask;
        }

        public async Task Write(string user, AutoSave autoSave)
        {
            using var fs = File.Create(_path, 131072, FileOptions.Asynchronous);
            var jsonOptions = new JsonSerializerOptions { WriteIndented = true };

            await JsonSerializer.SerializeAsync<AutoSave>(fs, autoSave, jsonOptions);
        }
    }
}