using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

namespace Flasher.Integration.Tests.Authentication.Login;

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

    // We don't suppress the hint because the best way to fix it would be to make the test
    // class sealed. But the test runner can't deal with that right now.
    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Theory]
    [InlineData(42)]
    [InlineData(666)]
    public async Task LoginWithExistingUser(int tokenLifetime)
    {
        var lifeTimeString = TimeSpan.FromSeconds(tokenLifetime);

        var inMemorySettings = new Dictionary<string, string?>
        {
            { "Authentication:TokenLifetime", $"{lifeTimeString}" },
            { "FileStore:Directory", _fileStoreDirectory },
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

        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage loginResponse = await client.Login(UserName, Password);
        IEnumerable<string> cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);
        using HttpResponseMessage apiResponse = await CallApi(client);

        _ = await Verify(loginResponse)
            .AppendValue("apiResponse", apiResponse)
            .UseParameters(tokenLifetime)
            .ScrubHostJwt();
    }

    [Fact]
    public async Task CallApiWithCookieSignedWithWrongKey()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        IEnumerable<string> cookies = await CookieSignedWithDifferentSecurityKey();
        client.AddCookies(cookies);
        using HttpResponseMessage response = await CallApi(client);

        _ = await Verify(response).ScrubHostJwt();
    }

    [Fact]
    public async Task LoginWithNonExistingUser()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login("jane@doe", Password);

        _ = await Verify(response);
    }

    [Fact]
    public async Task LoginWithWrongPassword()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login(UserName, "123457");

        _ = await Verify(response);
    }

    [Fact]
    public async Task MissingDirectoryOption()
    {
        var inMemorySettings = new Dictionary<string, string?> { { "FileStore:Directory", null } };
        using WebApplicationFactory<Program> factory =
            new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
                builder.ConfigureAppConfiguration(
                    (context, conf) =>
                    {
                        _ = conf.AddInMemoryCollection(inMemorySettings);
                    }
                )
            );
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login(UserName, Password);

        _ = await Verify(response);
    }

    [Fact]
    public async Task UsersFileYieldsNoDictionary()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();
        string profilePath = Path.Combine(_fileStoreDirectory, UserName, "profile.json");
        File.WriteAllText(profilePath, "null");

        using HttpResponseMessage response = await client.Login(UserName, Password);

        _ = await Verify(response);
    }

    [Fact]
    public async Task OptionsAreInjected()
    {
        var inMemorySettings = new Dictionary<string, string?>
        {
            { "Authentication:TokenLifetime", "0.00:00:42" },
            { "FileStore:Directory", _fileStoreDirectory },
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
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login(UserName, Password);

        // Check that the token's Max-Age is not 0. Because 0 corresponds to the default
        // TimeSpan, which would be used if the property were not explicitly configured.
        _ = await Verify(response).ScrubHostJwt();
    }

    private async Task<IEnumerable<string>> CookieSignedWithDifferentSecurityKey()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();
        using HttpResponseMessage response = await client.Login(UserName, Password);
        return response.GetCookies();
    }

    private WebApplicationFactory<Program> CreateWebApplicationFactory()
    {
        return Util.CreateWebApplicationFactory(_fileStoreDirectory);
    }

    private static Task<HttpResponseMessage> CallApi(HttpClient client)
    {
        return client.GetAsync("/Cards/Next");
    }
}
