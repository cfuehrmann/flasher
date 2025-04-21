using System.Net.Http.Json;
using System.Text.Json;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

namespace Flasher.Integration.Tests.History;

public sealed class Delete : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public Delete()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task ShouldReturnOkWhenCardFound()
    {
        var newCardWaitingTime = new TimeSpan(1, 23, 45, 678);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", newCardWaitingTime.ToString() },
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

        using var postResponse = await client.PostAsJsonAsync(
            "/Cards",
            new { prompt = "prompt", solution = "solution" }
        );
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var timeBeforeDelete = DateTime.Now;

        using var response = await client.DeleteAsync($"History/{postResponseId}");

        var timeAfterDelete = DateTime.Now;

        var responseString = await response.Content.ReadAsStringAsync();
        using var responseDocument = JsonDocument.Parse(responseString);
        var card = responseDocument.RootElement;
        var changeTime = card.GetProperty("changeTime").GetDateTime();
        var nextTime = card.GetProperty("nextTime").GetDateTime();

        _ = await Verify(
            new
            {
                postResponse,
                response,
                ChangeTimeOk = timeBeforeDelete <= changeTime && changeTime <= timeAfterDelete,
                NextTimeOk = nextTime == changeTime + newCardWaitingTime,
            }
        );
    }

    [Fact]
    public async Task DeleteForNonExistingCard()
    {
        var newCardWaitingTime = new TimeSpan(1, 23, 45, 678);

        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:NewCardWaitingTime", newCardWaitingTime.ToString() },
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

        using var response = await client.DeleteAsync(
            $"History/0d305cfd-9a33-46cd-807b-8adefbe57e42"
        );

        _ = await Verify(new { response });
    }
}
