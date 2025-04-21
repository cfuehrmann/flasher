using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

namespace Flasher.Integration.Tests.Cards.Id;

public sealed class Patch : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public Patch()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Theory]
    [InlineData( /*lang=json,strict*/
        """{ "prompt": "prompt2" }"""
    )]
    [InlineData( /*lang=json,strict*/
        """{ "solution": "solution2" }"""
    )]
    public async Task ReadImmediately(string requestString)
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

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        using var postRequest = JsonContent.Create(
            new { prompt = "prompt", solution = "solution" }
        );
        using var postResponse = await client.PostAsync("/Cards", postRequest);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var requestContent = new StringContent(requestString, Encoding.UTF8, "application/json");
        using var response = await client.PatchAsync($"Cards/{postResponseId}", requestContent);

        using var getResponse = await client.GetAsync($"/Cards");

        _ = await Verify(new { postResponse, getResponse }).UseParameters(requestString);
    }

    [Fact]
    public async Task ReadAfterApplicationRestart()
    {
        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
        };

        using var factory1 = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    _ = conf.AddInMemoryCollection(settings);
                }
            )
        );

        var clientBeforeRestart = factory1.CreateClient();

        using var loginResponseBeforeRestart = await clientBeforeRestart.Login(UserName, Password);
        var cookiesBeforeRestart = loginResponseBeforeRestart.GetCookies();
        clientBeforeRestart.AddCookies(cookiesBeforeRestart);

        using var postRequest = JsonContent.Create(
            new { prompt = "prompt", solution = "solution" }
        );
        using var postResponse = await clientBeforeRestart.PostAsync("/Cards", postRequest);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        var request = JsonContent.Create(new { prompt = "prompt2" });
        using var response = await clientBeforeRestart.PatchAsync(
            $"Cards/{postResponseId}",
            request
        );

        using var getResponseBeforeRestart = await clientBeforeRestart.GetAsync($"/Cards");

        factory1.Dispose();

        using var factory2 = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    _ = conf.AddInMemoryCollection(settings);
                }
            )
        );

        var clientAfterRestart = factory2.CreateClient();

        using var loginResponseAfterRestart = await clientAfterRestart.Login(UserName, Password);
        var cookiesAfterRestart = loginResponseAfterRestart.GetCookies();
        clientAfterRestart.AddCookies(cookiesAfterRestart);

        using var getResponseAfterRestart = await clientAfterRestart.GetAsync($"/Cards");

        _ = await Verify(
            new
            {
                postResponse,
                getResponseBeforeRestart,
                getResponseAfterRestart,
            }
        );
    }

    [Fact]
    public async Task ShouldReturnOkWhenCardFound()
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

        using var postResponse = await client.PostAsJsonAsync(
            "/Cards",
            new { prompt = "prompt", solution = "solution" }
        );
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        using var response = await client.PatchAsJsonAsync($"Cards/{postResponseId}", new { });

        _ = await Verify(new { response });
    }

    [Fact]
    public async Task ShouldDeleteAutoSave()
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

        var request = new { prompt = "prompt", solution = "solution" };

        using var postResponse = await client.PostAsJsonAsync("/Cards", request);
        var postResponseString = await postResponse.Content.ReadAsStringAsync();
        using var postResponseDocument = JsonDocument.Parse(postResponseString);
        var postResponseId = postResponseDocument.RootElement.GetProperty("id").GetString();

        using HttpResponseMessage autoSaveResponse = await client.PutAsJsonAsync(
            "/AutoSave",
            new
            {
                id = "some-id",
                prompt = "p",
                solution = "s",
            }
        );

        using var response = await client.PatchAsJsonAsync($"Cards/{postResponseId}", request);

        using var loginResponse2 = await client.Login(UserName, Password);
        using var getResponse = await client.GetAsync("/Cards");

        _ = await Verify(
            new
            {
                autoSaveResponse,
                postResponse,
                response,
                loginResponse2,
                getResponse,
            }
        );
    }

    [Fact]
    public async Task PatchNonExistingCard()
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

        using var loginResponse = await client.Login(UserName, Password);
        var cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);

        using var response = await client.PatchAsJsonAsync(
            "Cards/0d305cfd-9a33-46cd-807b-8adefbe57e42",
            new { prompt = "somePrompt", solution = "someSolution" }
        );

        _ = await Verify(new { response });
    }
}
