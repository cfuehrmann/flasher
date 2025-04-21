using System.Text.Json;
using System.Text.Json.Serialization;
using Flasher.Store.AutoSaving;
using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.AutoSaving;

public class AutoSaveStore(
    IOptionsMonitor<FileStoreOptions> options,
    IFileStoreJsonContextProvider jsonContextProvider
) : IAutoSaveStore
{
    private readonly JsonSerializerContext _jsonContext = jsonContextProvider.Instance;

    public async Task<AutoSave?> Read(string user)
    {
        string path = GetPath(user);

        if (!File.Exists(path))
        {
            return null;
        }

        using FileStream fs = File.OpenRead(path);
        var type = typeof(SerializableAutoSave);
        object? deserialized = await JsonSerializer.DeserializeAsync(fs, type, _jsonContext);
        if (deserialized is not SerializableAutoSave typedValue)
        {
            throw new InvalidOperationException("Deserializing the auto save file returned null!");
        }

        string id =
            typedValue.Id
            ?? throw new InvalidOperationException($"The Id of the auto save is null!");
        string prompt =
            typedValue.Prompt
            ?? throw new InvalidOperationException($"The Prompt of the auto save is null!");
        string solution =
            typedValue.Solution
            ?? throw new InvalidOperationException($"The Solution of the auto save is null!");
        return new AutoSave
        {
            Id = id,
            Prompt = prompt,
            Solution = solution,
        };
    }

    public Task Delete(string user)
    {
        string path = GetPath(user);
        try
        {
            using var fs = new FileStream(
                path,
                FileMode.Open,
                FileAccess.Read,
                FileShare.None,
                1,
                FileOptions.DeleteOnClose
            );
        }
        catch (FileNotFoundException) { }
        return Task.CompletedTask;
    }

    public async Task Write(string user, AutoSave autoSave)
    {
        string path = GetPath(user);
        using var fs = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.None);

        var s = new SerializableAutoSave
        {
            Id = autoSave.Id,
            Prompt = autoSave.Prompt,
            Solution = autoSave.Solution,
        };

        await JsonSerializer.SerializeAsync(fs, s, typeof(SerializableAutoSave), _jsonContext);
    }

    private string GetPath(string user)
    {
        return Path.Combine(options.CurrentValue.Directory, user, "autoSave.json");
    }
}
