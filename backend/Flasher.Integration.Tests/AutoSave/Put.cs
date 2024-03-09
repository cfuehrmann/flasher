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

namespace Flasher.Integration.Tests.AutoSave;

public sealed class Put : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public Put()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task ShouldPutAutoSaveInLoginResponse()
    {
        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory }
        };

        using var factory = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
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

        var cardId = "someCardId";
        var prompt = "somePrompt";
        var solution = "someSolution";

        using HttpResponseMessage response = await client.PutAsync(
            "/AutoSave",
            new StringContent(
                $$"""{ "id": "{{cardId}}", "prompt": "{{prompt}}", "solution": "{{solution}}" }""",
                Encoding.UTF8,
                "application/json"
            )
        );

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);

        using var loginResponse2 = await client.Login(UserName, Password);
        var loginResponse2String = await loginResponse2.Content.ReadAsStringAsync();
        using var loginResponse2Document = JsonDocument.Parse(loginResponse2String);

        var autoSave = loginResponse2Document.RootElement.GetProperty("autoSave");
        var autoSaveId = autoSave.GetProperty("id").GetString();
        Assert.Equal(cardId, autoSaveId);
        var autoSavePrompt = autoSave.GetProperty("prompt").GetString();
        Assert.Equal(prompt, autoSavePrompt);
        var autoSaveSolution = autoSave.GetProperty("solution").GetString();
        Assert.Equal(solution, autoSaveSolution);
    }
}
