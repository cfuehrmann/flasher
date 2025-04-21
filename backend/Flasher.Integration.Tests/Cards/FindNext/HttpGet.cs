using System.Diagnostics;
using System.Net;
using System.Net.Http.Json;
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

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        using var postResponse = await client.PostAsync(
            "/Cards",
            JsonContent.Create(new { prompt = "some prompt", solution = "some solution" })
        );

        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var postNextTime = postResponseDocument
            .RootElement.GetProperty("nextTime")
            .GetDateTimeOffset();

        var enableResponse = await client.PostAsync($"/Cards/{postId}/Enable", null);

        var time = Stopwatch.GetTimestamp();

        while (Stopwatch.GetElapsedTime(time) < TimeSpan.FromMilliseconds(200))
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

            await Task.Delay(delay);
        }

        Assert.Fail("Timeout!");
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

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        using var postResponse = await client.PostAsync(
            "/Cards",
            JsonContent.Create(new { prompt = "prompt", solution = "solution" })
        );

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

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        using var response = await client.GetAsync("/Cards/Next");

        _ = await Verify(new { response });
    }

    [Fact]
    public async Task ShouldFindEarliestCard()
    {
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

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        using var postResponse1 = await client.PostAsync(
            "/Cards",
            JsonContent.Create(new { prompt = "prompt1", solution = "solution1" })
        );
        var postResponseString1 = await postResponse1.Content.ReadAsStringAsync();
        using var postResponseDocument1 = JsonDocument.Parse(postResponseString1);
        var postId1 = postResponseDocument1.RootElement.GetProperty("id").GetString();
        _ = await client.PostAsync($"/Cards/{postId1}/Enable", null);

        using var postResponse2 = await client.PostAsync(
            "/Cards",
            JsonContent.Create(new { prompt = "prompt2", solution = "solution2" })
        );
        var postResponseString2 = await postResponse2.Content.ReadAsStringAsync();
        using var postResponseDocument2 = JsonDocument.Parse(postResponseString2);
        var postId2 = postResponseDocument2.RootElement.GetProperty("id").GetString();
        _ = await client.PostAsync($"/Cards/{postId2}/Enable", null);
        var post2NextTime = postResponseDocument2
            .RootElement.GetProperty("nextTime")
            .GetDateTimeOffset();

        // Wait until the next time the second card is due.
        // Strangely, Task.Delay(postNextTime - now) sometimes does not wait
        // long enough.
        var time = Stopwatch.GetTimestamp();

        while (true)
        {
            var now = DateTimeOffset.Now;

            if (now > post2NextTime)
            {
                break;
            }

            if (Stopwatch.GetElapsedTime(time) > TimeSpan.FromMilliseconds(200))
            {
                Assert.Fail("Timeout!");
            }
            ;

            await Task.Delay(10);
        }

        using var response = await client.GetAsync("/Cards/Next");

        _ = await Verify(new { postResponse1, response });
    }
}
