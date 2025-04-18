﻿using System.Net;
using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

namespace Flasher.Integration.Tests.Cards.FindNext;

public sealed class HttpGet : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public HttpGet()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task ShouldFindCardExactlyWhenDue()
    {
        var delay = TimeSpan.FromMilliseconds(10);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:00.100" },
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

        var postPrompt = "some prompt";
        var postSolution = "some solution";

        var postBodyString = $$"""
            {
                "prompt": "{{postPrompt}}",
                "solution": "{{postSolution}}"
            }
            """;
        var postBodyContent = new StringContent(postBodyString, Encoding.UTF8, "application/json");
        using var postResponse = await client.PostAsync("/Cards", postBodyContent);

        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var postNextTime = postResponseDocument
            .RootElement.GetProperty("nextTime")
            .GetDateTimeOffset();

        var enableResponse = await client.PostAsync($"/Cards/{postId}/Enable", null);

        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        while (true)
        {
            using var response = await client.GetAsync("/Cards/Next");

            if (response.StatusCode == HttpStatusCode.OK)
            {
                DateTime now = DateTime.Now;

                _ = await Verify(
                    new
                    {
                        postResponse,
                        NextTimeInRange = postNextTime <= now
                            && now <= postNextTime + delay + TimeSpan.FromMilliseconds(10),
                        response,
                    }
                );

                return;
            }

            Assert.Equal(HttpStatusCode.NoContent, response.StatusCode);

            await Task.Delay(delay);
        }
    }

    [Fact]
    public async Task ShouldNotFindDisabledCard()
    {
        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:00" },
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

        var postBodyString = $$"""
            {
                "prompt": "prompt",
                "solution": "solution"
            }
            """;
        var postBodyContent = new StringContent(postBodyString, Encoding.UTF8, "application/json");
        using var postResponse = await client.PostAsync("/Cards", postBodyContent);

        using var response = await client.GetAsync("/Cards/Next");

        _ = await Verify(new { response });
    }

    [Fact]
    public async Task ShouldReturnNoContentOnEmptyStore()
    {
        var delay = TimeSpan.FromMilliseconds(10);

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

        using var response = await client.GetAsync("/Cards/Next");

        _ = await Verify(new { response });
    }
}
