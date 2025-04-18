using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

namespace Flasher.Integration.Tests.Cards.Id;

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
    public async Task ShouldMakeCardUnfindable()
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

        using var response = await client.DeleteAsync($"Cards/{postResponseId}");

        using var getResponse = await client.GetAsync("/Cards");

        _ = await Verify(new { response, getResponse });
    }
}
