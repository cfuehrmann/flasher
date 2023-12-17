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

public sealed class Disable : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public Disable()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task ShouldDisableCard()
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

        var postRequestString = $$"""
            {
                "prompt": "prompt",
                "solution": "solution"
            }
            """;
        var postRequestContent = new StringContent(
            postRequestString,
            Encoding.UTF8,
            "application/json"
        );
        using var postResponse = await client.PostAsync("/Cards", postRequestContent);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var enableResponse = await client.PostAsync($"/Cards/{postResponseId}/Enable", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        using var response = await client.PostAsync($"Cards/{postResponseId}/Disable", null);
        Assert.Equal(HttpStatusCode.NoContent, response.StatusCode);

        using var getResponse = await client.GetAsync("/Cards");
        var getResponseString = await getResponse.Content.ReadAsStringAsync();
        using var getResponseDocument = JsonDocument.Parse(getResponseString);
        var getCard = getResponseDocument.RootElement.GetProperty("cards")[0];
        var getDisabled = getCard.GetProperty("disabled").GetBoolean();
        Assert.True(getDisabled);
    }

    [Fact]
    public async Task ShouldReturnNotFoundWhenCardNotFound()
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

        var response = await client.PostAsync($"/Cards/nonExistingId/Disable", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
    }
}
