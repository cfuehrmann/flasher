using System;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;

using Flasher.Store.AutoSaving;

using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.AutoSaving;

public class AutoSaveStore : IAutoSaveStore
{
    private readonly string _directory;
    private readonly JsonSerializerContext _jsonContext;

    public AutoSaveStore(IOptionsMonitor<FileStoreOptions> options, IFileStoreJsonContextProvider jsonContextProvider)
    {
        _jsonContext = jsonContextProvider.Instance;
        _directory = options.CurrentValue.Directory ??
            throw new ArgumentException("Missing configuration 'FileStore:Directory'");
    }

    public async Task<AutoSave?> Read(string user)
    {
        var path = GetPath(user);
        if (!File.Exists(path)) return null;
        using var fs = File.OpenRead(path);
        var type = typeof(SerializableAutoSave);
        var deserialized = await JsonSerializer.DeserializeAsync(fs, type, _jsonContext);
        if (deserialized is not SerializableAutoSave typedValue)
            throw new InvalidOperationException("Deserializing the auto save file returned null!");
        var id = typedValue.Id ??
            throw new InvalidOperationException($"The Id of the auto save is null!");
        var prompt = typedValue.Prompt ??
            throw new InvalidOperationException($"The Prompt of the auto save is null!");
        var solution = typedValue.Solution ??
            throw new InvalidOperationException($"The Solution of the auto save is null!");
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
        using var fs = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.None, 131072, true);
        var (id, prompt, solution) = autoSave;
        var s = new SerializableAutoSave { Id = id, Prompt = prompt, Solution = solution };
        await JsonSerializer.SerializeAsync(fs, s, typeof(SerializableAutoSave), _jsonContext);
    }

    private string GetPath(string user) => Path.Combine(_directory, user, "autoSave.json");
}
