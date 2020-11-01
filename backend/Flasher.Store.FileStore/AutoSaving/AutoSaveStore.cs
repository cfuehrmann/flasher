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

        public AutoSaveStore(IOptionsMonitor<FileStoreOptions> options) =>
            _directory = options.CurrentValue.Directory ?? throw new("Missing configuration 'FileStore:Directory'");

        public async Task<AutoSave?> Read(string user)
        {
            var path = GetPath(user);
            if (!File.Exists(path)) return null;
            using var fs = File.OpenRead(path);
            var deserialized = await JsonSerializer.DeserializeAsync<DeserializedAutoSave>(fs) ??
                throw new("Deserializing the auto save file returned null!");
            var id = deserialized.id ?? throw new($"The Id of the auto save is null!");
            var prompt = deserialized.prompt ?? throw new($"The Prompt of the auto save is null!");
            var solution = deserialized.solution ?? throw new($"The Solution of the auto save is null!");
            return new AutoSave(id, prompt, solution);
        }

        public Task Delete(string user)
        {
            var path = GetPath(user);
            try
            {
                using var fs = new FileStream(path, FileMode.Open, FileAccess.Read,
                    FileShare.None, 1, FileOptions.DeleteOnClose);
            }
            catch (FileNotFoundException) { }
            return Task.CompletedTask;
        }

        public async Task Write(string user, AutoSave autoSave)
        {
            var path = GetPath(user);
            using var fs = new FileStream(path, FileMode.Create, FileAccess.Write,
                FileShare.None, 131072, true);
            var jsonOptions = new JsonSerializerOptions { WriteIndented = true };
            await JsonSerializer.SerializeAsync<AutoSave>(fs, autoSave, jsonOptions);
        }

        private string GetPath(string user) => Path.Combine(_directory, user, "autoSave.json");
    }
}