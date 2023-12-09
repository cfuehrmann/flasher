﻿using System;
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

public sealed class SetOk : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public SetOk()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task Smoke()
    {
        var multiplier = 1.8;
        var minWaitingTime = TimeSpan.FromMilliseconds(100);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:PageSize", "99" },
            { "Cards:NewCardWaitingTime", "00:00:00" },
            { "Cards:OkMultiplier", $"{multiplier}" }
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

        HttpClient client = factory.CreateClient();
        client.Timeout = TimeSpan.FromSeconds(1);

        using HttpResponseMessage loginResponse = await client.Login(UserName, Password);
        IEnumerable<string> cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        var postBodyString = $$"""
            {
                "prompt": "prompt",
                "solution": "solution"
            }
            """;
        var postBodyContent = new StringContent(postBodyString, Encoding.UTF8, "application/json");
        using HttpResponseMessage postResponse = await client.PostAsync("/Cards", postBodyContent);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using JsonDocument doc = JsonDocument.Parse(postResponseString);
        var cardId = doc.RootElement.GetProperty("id").GetString();
        var postChangeTime = doc.RootElement.GetProperty("changeTime").GetDateTime();

        var enableTask = client.PostAsync($"/Cards/{cardId}/Enable", null);

        await Task.WhenAll(
            [enableTask, Task.Delay(postChangeTime + minWaitingTime - DateTime.Now)]
        );

        var enableResponse = await enableTask;

        // To prevent Stryker timeouts
        Assert.Equal(HttpStatusCode.NoContent, enableResponse.StatusCode);

        using HttpResponseMessage response = await client.PostAsync($"/Cards/{cardId}/SetOk", null);

        var fromPostUntilSetOk = DateTime.Now - postChangeTime;

        Console.WriteLine($"Milliseconds from POST until SetOk responded: {fromPostUntilSetOk}");

        var maxWaitingTime = fromPostUntilSetOk * multiplier;

        Console.WriteLine($"Maximum expected waiting time for the new card is {maxWaitingTime}");

        Assert.Equal(HttpStatusCode.NoContent, response.StatusCode);

        await Task.Delay(maxWaitingTime);

        using HttpResponseMessage nextResponse = await client.GetAsync("/Cards/Next");

        Assert.Equal(HttpStatusCode.OK, nextResponse.StatusCode);

        var nextResponseString = await nextResponse.Content.ReadAsStringAsync();
        using JsonDocument nextResponseJson = JsonDocument.Parse(nextResponseString);
        var changeTime = nextResponseJson.RootElement.GetProperty("changeTime").GetDateTime();
        var nextTime = nextResponseJson.RootElement.GetProperty("nextTime").GetDateTime();

        TimeSpan waitingTime = nextTime - changeTime;

        Console.WriteLine($"Waiting time of the new card: {waitingTime})");

        Assert.True(minWaitingTime * multiplier <= waitingTime);
        Assert.True(waitingTime <= maxWaitingTime);
    }
}
