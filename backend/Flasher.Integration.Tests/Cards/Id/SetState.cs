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
    [InlineData("Ok", 0)] // Limit case where the card is immediately due again.
    public async Task Smoke(string state, double multiplier)
    {
        var minWaitingTime = TimeSpan.FromMilliseconds(100);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:PageSize", "99" },
            { "Cards:NewCardWaitingTime", "00:00:00" },
            { $"Cards:{state}Multiplier", $"{multiplier}" }
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
        var postChangeTime = document.RootElement.GetProperty("changeTime").GetDateTime();

        var enableTask = client.PostAsync($"/Cards/{cardId}/Enable", null);

        await Task.WhenAll(
            [enableTask, Task.Delay(postChangeTime + minWaitingTime - DateTime.Now)]
        );

        var enableResponse = await enableTask;

        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        using var response = await client.PostAsync($"/Cards/{cardId}/Set{state}", null);

        var fromPostUntilSetState = DateTime.Now - postChangeTime;

        Console.WriteLine(
            $"Time elapsed between POST and response of SetState: {fromPostUntilSetState}"
        );

        var maxWaitingTime = fromPostUntilSetState * multiplier;

        Console.WriteLine($"Maximum expected waiting time for the new card is {maxWaitingTime}");

        // The status code can be NoContent, or OK if the nextTime is so close
        // that the card is arleady due.
        Assert.True(response.IsSuccessStatusCode);

        await Task.Delay(maxWaitingTime);

        using var nextResponse = await client.GetAsync("/Cards/Next");

        Assert.Equal(HttpStatusCode.OK, nextResponse.StatusCode);

        var nextResponseString = await nextResponse.Content.ReadAsStringAsync();
        using var nextResponseJson = JsonDocument.Parse(nextResponseString);
        var changeTime = nextResponseJson.RootElement.GetProperty("changeTime").GetDateTime();
        var nextTime = nextResponseJson.RootElement.GetProperty("nextTime").GetDateTime();
        var waitingTime = nextTime - changeTime;
        Console.WriteLine($"Waiting time of the new card: {waitingTime})");

        Assert.True(minWaitingTime * multiplier <= waitingTime);
        Assert.True(waitingTime <= maxWaitingTime);
    }
}
