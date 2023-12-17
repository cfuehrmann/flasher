using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;
using Xunit;

namespace Flasher.Integration.Tests.Cards.Id;

public sealed class Patch : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public Patch()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task ShouldReturnOkWhenCardFound()
    {
        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory }
        };

        using var factory = new WebApplicationFactory<Program>().WithWebHostBuilder(
            builder =>
                builder.ConfigureAppConfiguration(
                    (context, conf) =>
                    {
                        _ = conf.AddInMemoryCollection(settings);
                    }
                )
        );

        var client = factory.CreateClient();
        client.Timeout = TimeSpan.FromSeconds(1);

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        var postBodyString = $$"""
            {
                "prompt": "prompt",
                "solution": "solution"
            }
            """;
        var postBodyContent = new StringContent(postBodyString, Encoding.UTF8, "application/json");
        using var postResponse = await client.PostAsync("/Cards", postBodyContent);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var document = JsonDocument.Parse(postResponseString);
        var cardId = document.RootElement.GetProperty("id").GetString();

        var patchBodyString = $$"""{}""";
        var patchBodyContent = new StringContent(
            patchBodyString,
            Encoding.UTF8,
            "application/json"
        );
        using var response = await client.PatchAsync($"Cards/{cardId}", patchBodyContent);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    [Fact]
    public async Task ShouldDeleteAutoSave()
    {
        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory }
        };

        using var factory = new WebApplicationFactory<Program>().WithWebHostBuilder(
            builder =>
                builder.ConfigureAppConfiguration(
                    (context, conf) =>
                    {
                        _ = conf.AddInMemoryCollection(settings);
                    }
                )
        );

        var client = factory.CreateClient();
        client.Timeout = TimeSpan.FromSeconds(1);

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        var postBodyString = $$"""
            {
                "prompt": "prompt",
                "solution": "solution"
            }
            """;
        var postBodyContent = new StringContent(postBodyString, Encoding.UTF8, "application/json");
        using var postResponse = await client.PostAsync("/Cards", postBodyContent);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var document = JsonDocument.Parse(postResponseString);
        var cardId = document.RootElement.GetProperty("id").GetString();

        using HttpResponseMessage autoSaveResponse = await client.PutAsync(
            "/AutoSave",
            new StringContent(
                $$"""{ "id": "some-id", "prompt": "p", "solution": "s" }""",
                Encoding.UTF8,
                "application/json"
            )
        );
        Assert.Equal(HttpStatusCode.OK, autoSaveResponse.StatusCode);

        var patchBodyString = $$"""{}""";
        var patchBodyContent = new StringContent(
            patchBodyString,
            Encoding.UTF8,
            "application/json"
        );
        using var response = await client.PatchAsync($"Cards/{cardId}", patchBodyContent);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);

        using var loginResponse2 = await client.Login(UserName, Password);
        var loginResponse2String = await loginResponse2.Content.ReadAsStringAsync();
        Console.WriteLine(loginResponse2String);
        using var autoSaveDocument = JsonDocument.Parse(loginResponse2String);

        var hasAutoSave = autoSaveDocument.RootElement.TryGetProperty("autoSave", out var autoSave);

        if (hasAutoSave)
        {
            Assert.Equal(JsonValueKind.Null, autoSave.ValueKind);
        }
    }
}
