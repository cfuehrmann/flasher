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

public sealed class SetState : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public SetState()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Theory]
    [InlineData("Ok", 1.8)]
    [InlineData("Failed", 0.5555)]
    [InlineData("Ok", 0)] // This is the limit case where the card is immediately due again.
    public async Task ShouldResultInCorrectWaitingTime(string state, double multiplier)
    {
        var delay = TimeSpan.FromMilliseconds(100);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:00" },
            { $"Cards:{state}Multiplier", $"{multiplier}" }
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
        var postRequestContent = new StringContent(
            postBodyString,
            Encoding.UTF8,
            "application/json"
        );
        using var postResponse = await client.PostAsync("/Cards", postRequestContent);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();
        var postResponseChangeTime = postResponseDocument
            .RootElement.GetProperty("changeTime")
            .GetDateTime();

        var enableResponse = await client.PostAsync($"/Cards/{postResponseId}/Enable", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        await Task.Delay(delay);

        using var response = await client.PostAsync($"/Cards/{postResponseId}/Set{state}", null);
        // The status code can be NoContent, or OK if the nextTime is so close
        // that the card is already due.
        Assert.True(response.IsSuccessStatusCode);

        using var getResponse = await client.GetAsync("/Cards");
        Assert.Equal(HttpStatusCode.OK, getResponse.StatusCode);
        var getResponseString = await getResponse.Content.ReadAsStringAsync();
        using var getResponseDocument = JsonDocument.Parse(getResponseString);
        var getCard = getResponseDocument.RootElement.GetProperty("cards")[0];
        var getChangeTime = getCard.GetProperty("changeTime").GetDateTime();
        var getNextTime = getCard.GetProperty("nextTime").GetDateTime();

        TimeSpan passedTime = getChangeTime - postResponseChangeTime;

        Assert.True(passedTime >= delay);

        var waitingTime = getNextTime - getChangeTime;

        Assert.Equal(passedTime * multiplier, waitingTime);
    }

    [Theory]
    [InlineData("Ok")]
    [InlineData("Failed")]
    public async Task ShouldReturnNotFoundWhenCardNotFound(string state)
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

        using var response = await client.PostAsync($"/Cards/nonExistingId/Set{state}", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
    }

    [Theory]
    [InlineData("Ok", 1.8)]
    [InlineData("Failed", 0.5555)]
    public async Task ShouldReturnNoContentWhenNoNextCard(string state, double multiplier)
    {
        var delay = TimeSpan.FromMilliseconds(100);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:00" },
            { $"Cards:{state}Multiplier", $"{multiplier}" }
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
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var enableResponse = await client.PostAsync($"/Cards/{postResponseId}/Enable", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        await Task.Delay(delay);

        using var response = await client.PostAsync($"/Cards/{postResponseId}/Set{state}", null);
        Assert.Equal(HttpStatusCode.NoContent, response.StatusCode);
    }

    [Theory]
    [InlineData("Ok", 1.8)]
    [InlineData("Failed", 0.5555)]
    public async Task ShouldReturnOkWhenNextCard(string state, double multiplier)
    {
        var delay = TimeSpan.FromMilliseconds(100);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:00" },
            { $"Cards:{state}Multiplier", $"{multiplier}" }
        };

        using var factory = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    var _configurationBuilder = conf.AddInMemoryCollection(settings);
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
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        using var postResponse2 = await client.PostAsync("/Cards", postBodyContent);
        var postResponse2String = await postResponse2.Content.ReadAsStringAsync();
        using var postResponse2Document = JsonDocument.Parse(postResponse2String);
        var postResponse2Id = postResponse2Document.RootElement.GetProperty("id").GetString();

        var enableResponse = await client.PostAsync($"/Cards/{postResponseId}/Enable", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        var _enableResponse2 = await client.PostAsync($"/Cards/{postResponse2Id}/Enable", null);
        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        await Task.Delay(delay);

        using var response = await client.PostAsync($"/Cards/{postResponseId}/Set{state}", null);
        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }
}
