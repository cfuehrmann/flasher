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

namespace Flasher.Integration.Tests.Cards;

public sealed class HttpPost : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    // The hash comes from the password. Don't let this test suite compute the hash, because
    // these tests should also protect against invalidating the password hash by accidental
    // change of the hash algorithm.
    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public HttpPost()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task Create()
    {
        var inMemorySettings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:42" }
        };

        using WebApplicationFactory<Program> factory =
            new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
                builder.ConfigureAppConfiguration(
                    (context, conf) =>
                    {
                        _ = conf.AddInMemoryCollection(inMemorySettings);
                    }
                )
            );

        using HttpClient client = await factory.Login(UserName, Password);

        var prompt = "somePrompt";
        var solution = "someSolution";

        var now = DateTime.Now;

        using HttpResponseMessage response = await client.PostAsync(
            "/Cards",
            new StringContent(
                $$"""{ "prompt": "{{prompt}}", "solution": "{{solution}}" }""",
                Encoding.UTF8,
                "application/json"
            )
        );

        Assert.Equal(HttpStatusCode.Created, response.StatusCode);

        using var getResponse = await client.GetAsync("/Cards");
        Assert.Equal(HttpStatusCode.OK, getResponse.StatusCode);
        var getResponseString = await getResponse.Content.ReadAsStringAsync();
        using var getResponseDocument = JsonDocument.Parse(getResponseString);
        var getCard = getResponseDocument.RootElement.GetProperty("cards")[0];

        var getId = getCard.GetProperty("id").GetString();
        Assert.NotNull(getId);
        Assert.True(getId.Length > 0);

        var getPrompt = getCard.GetProperty("prompt").GetString();
        Assert.Equal(prompt, getPrompt);

        var getSolution = getCard.GetProperty("solution").GetString();
        Assert.Equal(solution, getSolution);

        var getState = getCard.GetProperty("state").GetString();
        Assert.Equal("New", getState);

        var getChangeTime = getCard.GetProperty("changeTime").GetDateTime();
        Assert.True(getChangeTime >= now);

        var getNextTime = getCard.GetProperty("nextTime").GetDateTime();
        Assert.Equal(getChangeTime.AddSeconds(42), getNextTime);

        var getDisabled = getCard.GetProperty("disabled").GetBoolean();
        Assert.True(getDisabled);
    }
}
