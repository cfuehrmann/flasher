﻿using System.Text;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

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
            { "FileStore:Directory", _fileStoreDirectory },
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

        using var loginResponse2 = await client.Login(UserName, Password);

        _ = await Verify(
            new
            {
                loginResponse,
                response,
                loginResponse2,
            }
        );
    }
}
