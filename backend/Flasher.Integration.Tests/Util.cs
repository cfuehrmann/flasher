using System;
using System.Collections.Generic;
using System.IO;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;

using Flasher.Host.Model;
using Flasher.Store.FileStore;

using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;

namespace Flasher.Integration.Tests;

public static class Util
{
    public static string InventFileStoreDirectory()
    {
        string current = Directory.GetCurrentDirectory();
        string subDirectory = Guid.NewGuid().ToString();
        return Path.Combine(current, subDirectory);
    }

    public static void CreateUserStore(string fileStoreDirectory, string userName, string passwordHash)
    {
        string userPath = Path.Combine(fileStoreDirectory, userName);
        _ = Directory.CreateDirectory(userPath);
        string usersFileContent = $"{{ \"UserName\": \"{userName}\", \"PasswordHash\": \"{passwordHash}\" }}";
        File.WriteAllText(Path.Combine(userPath, "profile.json"), usersFileContent);
        File.WriteAllText(Path.Combine(userPath, "cards.json"), "[]");
    }

    public static void WriteCardsFile(string fileStoreDirectory, string userName, IEnumerable<string> cards)
    {
        string userPath = Path.Combine(fileStoreDirectory, userName);
        File.WriteAllText(Path.Combine(userPath, "cards.json"), $"[{string.Join(", ", cards)}]");
    }

    public static WebApplicationFactory<Program> CreateWebApplicationFactory(string fileStoreDirectory) =>
        new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureServices(services => services.Configure<FileStoreOptions>(option =>
                option.Directory = fileStoreDirectory)));

    public static async Task<HttpClient> Login(this WebApplicationFactory<Program> factory, string userName, string password)
    {
        HttpClient client = factory.CreateClient();
        using HttpResponseMessage loginResponse = await client.Login(userName, password);
        IEnumerable<string> cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);
        return client;
    }

    public static async Task<HttpResponseMessage> Login(this HttpClient client, string userName, string password)
    {
        return await client.PostAsJsonAsync("/Authentication/Login", new LoginRequest { UserName = userName, Password = password });
    }

    public static IEnumerable<string> GetCookies(this HttpResponseMessage response)
    {
        return response.Headers.GetValues("Set-Cookie");
    }

    public static void AddCookies(this HttpClient client, IEnumerable<string> cookies) =>
        client.DefaultRequestHeaders.Add("Cookie", cookies);

    public static async Task<T?> ReadJsonAsync<T>(this HttpResponseMessage response)
    {
        var jsonOptions = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
        jsonOptions.Converters.Add(new JsonStringEnumConverter());
        return await response.Content.ReadFromJsonAsync<T>(jsonOptions);
    }

    public static IEnumerable<T> Reverse<T>(bool reverse, T element0, T element1) =>
        reverse ? new[] { element0, element1 } : new[] { element1, element0 };
}
