﻿using System;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;
using Flasher.Store.AutoSaving;
using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.AutoSaving;

public class AutoSaveStore : IAutoSaveStore
{
    private readonly string _directory;

    public AutoSaveStore(IOptionsMonitor<FileStoreOptions> options) =>
        _directory = options.CurrentValue.Directory ??
            throw new ArgumentException("Missing configuration 'FileStore:Directory'");

    public async Task<AutoSave?> Read(string user)
    {
        var path = GetPath(user);
        if (!File.Exists(path)) return null;
        using var fs = File.OpenRead(path);
        var deserialized = await JsonSerializer.DeserializeAsync<DeserializedAutoSave>(fs) ??
            throw new InvalidOperationException("Deserializing the auto save file returned null!");
        var id = deserialized.Id ??
            throw new InvalidOperationException($"The Id of the auto save is null!");
        var prompt = deserialized.Prompt ??
            throw new InvalidOperationException($"The Prompt of the auto save is null!");
        var solution = deserialized.Solution ??
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
        using var fs = new FileStream(path, FileMode.Create, FileAccess.Write,
            FileShare.None, 131072, true);
        var jsonOptions = new JsonSerializerOptions { WriteIndented = true };
        await JsonSerializer.SerializeAsync(fs, autoSave, jsonOptions);
    }

    private string GetPath(string user) => Path.Combine(_directory, user, "autoSave.json");
}
