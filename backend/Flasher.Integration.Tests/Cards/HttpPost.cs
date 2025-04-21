using System.Net.Http.Json;
using System.Text.Json;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

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
            { "Cards:NewCardWaitingTime", "00:00:42" },
        };

        using var factory = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    _ = conf.AddInMemoryCollection(inMemorySettings);
                }
            )
        );

        using var client = await factory.Login(UserName, Password);

        var now = DateTime.Now;

        using var response = await client.PostAsync(
            "/Cards",
            JsonContent.Create(new { prompt = "somePrompt", solution = "someSolution" })
        );

        using var getResponse = await client.GetAsync("/Cards");

        var getResponseString = await getResponse.Content.ReadAsStringAsync();
        using var getResponseDocument = JsonDocument.Parse(getResponseString);
        var getCard = getResponseDocument.RootElement.GetProperty("cards")[0];
        var getChangeTime = getCard.GetProperty("changeTime").GetDateTime();
        var getNextTime = getCard.GetProperty("nextTime").GetDateTime();

        _ = await Verify(
            new
            {
                response,
                getResponse,
                ChangeTimeOk = getChangeTime >= now,
                NextTimeOk = getNextTime == getChangeTime.AddSeconds(42),
            }
        );
    }

    [Fact]
    public async Task ReadAfterApplicationRestart()
    {
        var inMemorySettings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", "00:00:42" },
        };

        using var factory1 = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    _ = conf.AddInMemoryCollection(inMemorySettings);
                }
            )
        );

        using var client1 = await factory1.Login(UserName, Password);

        using var response = await client1.PostAsync(
            "/Cards",
            JsonContent.Create(new { prompt = "somePrompt", solution = "someSolution" })
        );

        using var factory2 = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    _ = conf.AddInMemoryCollection(inMemorySettings);
                }
            )
        );

        using var client2 = await factory2.Login(UserName, Password);

        using var getResponse = await client2.GetAsync("/Cards");

        _ = await Verify(new { response, getResponse });
    }
}
