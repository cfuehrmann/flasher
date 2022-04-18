using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text.RegularExpressions;
using System.Threading.Tasks;

using Flasher.Host;
using Flasher.Store.FileStore;

using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;

using Xunit;

namespace Flasher.Integration.Tests.Authentication.Login;

public sealed class HttpPost : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";

    // The hash comes from the password. Don't let this test suite compute the hash, because
    // these tests should also protect against invalidating the password hash by accidental
    // change of the hash algorithm.    
    private const string PasswordHash =
        "AQAAAAEAACcQAAAAECUeTNmWxlWlEkteOikXzkwBM4VBrTYekVb9U+QBZjbcuk9V5ThbD4BfYDjzokwbVQ==";

    private readonly string _fileStoreDirectory;

    public HttpPost()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose() => Directory.Delete(_fileStoreDirectory, true);

    [Theory]
    [InlineData(42)]
    [InlineData(666)]
    public async Task LoginWithExistingUser(int tokenLifetime)
    {
        using WebApplicationFactory<Program> factory = new WebApplicationFactory<Program>()
            .WithWebHostBuilder(builder =>
                builder.ConfigureServices(services =>
                    services
                        .Configure((Action<AuthenticationOptions>)(option => option.TokenLifetime = TimeSpan.FromSeconds(tokenLifetime)))
                        .Configure((Action<FileStoreOptions>)(option => option.Directory = _fileStoreDirectory))));
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage loginResponse = await client.Login(UserName, Password);
        IEnumerable<string> cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);
        using HttpResponseMessage apiResponse = await CallApi(client);

        Assert.Equal(HttpStatusCode.OK, loginResponse.StatusCode);
        string? jwtCookie = cookies.FirstOrDefault(cookie => cookie.StartsWith("__Host-jwt", StringComparison.Ordinal));
        Assert.NotNull(jwtCookie);
        Assert.Matches(new Regex("; Path=/", RegexOptions.IgnoreCase), jwtCookie);
        Assert.Matches(new Regex("; Secure", RegexOptions.IgnoreCase), jwtCookie);
        Assert.Matches(new Regex("; HttpOnly", RegexOptions.IgnoreCase), jwtCookie);
        Assert.Matches(new Regex("; SameSite=strict", RegexOptions.IgnoreCase), jwtCookie);
        Assert.Matches(new Regex($"; Max-Age={tokenLifetime}", RegexOptions.IgnoreCase), jwtCookie);
        _ = apiResponse.EnsureSuccessStatusCode();
    }

    [Fact]
    public async Task CallApiWithCookieSignedWithWrongKey()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        IEnumerable<string> cookies = await CookieSignedWithDifferentSecurityKey();
        client.AddCookies(cookies);
        using HttpResponseMessage response = await CallApi(client);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task LoginWithNonExistingUser()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login("jane@doe", Password);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task LoginWithWrongPassword()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login(UserName, "123457");

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task MissingDirectoryOption()
    {
        using WebApplicationFactory<Program> factory = new WebApplicationFactory<Program>()
            .WithWebHostBuilder(builder =>
                builder.ConfigureServices(services =>
                    services.Configure<FileStoreOptions>(option => option.Directory = null)));
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login(UserName, Password);

        Assert.Equal(HttpStatusCode.InternalServerError, response.StatusCode);
        Assert.Equal(MediaTypeHeaderValue.Parse("text/html"), response.Content.Headers.ContentType);
        string content = await response.Content.ReadAsStringAsync();
        Assert.Matches(new Regex("store", RegexOptions.IgnoreCase), content);
        Assert.Matches(new Regex("configuration", RegexOptions.IgnoreCase), content);
        Assert.Matches(new Regex("filestore", RegexOptions.IgnoreCase), content);
        Assert.Matches(new Regex("directory", RegexOptions.IgnoreCase), content);
        Assert.EndsWith("!", content);
        Assert.False(response.Headers.Contains("Set-Cookie"));
    }

    [Fact]
    public async Task UsersFileYieldsNoDictionary()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();
        string profilePath = Path.Combine(_fileStoreDirectory, UserName, "profile.json");
        File.WriteAllText(profilePath, "null");

        using HttpResponseMessage response = await client.Login(UserName, Password);

        Assert.Equal(HttpStatusCode.InternalServerError, response.StatusCode);
        string content = await response.Content.ReadAsStringAsync();
        Assert.Matches(new Regex("file", RegexOptions.IgnoreCase), content);
        Assert.Matches(new Regex("invalid", RegexOptions.IgnoreCase), content);
        Assert.EndsWith("!", content);
        Assert.False(response.Headers.Contains("Set-Cookie"));
    }

    [Fact]
    public async Task OptionsAreInjected()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage loginResponse = await client.Login(UserName, Password);
        IEnumerable<string> cookies = loginResponse.GetCookies();

        string? jwtCookie = cookies.FirstOrDefault(cookie =>
            cookie.StartsWith("__Host-jwt", StringComparison.Ordinal));
        // Check that the token's Max-Age is not the default TimeSpan
        Assert.Matches(new Regex("; Max-Age=.*[1..9].*", RegexOptions.IgnoreCase), jwtCookie);
    }

    private async Task<IEnumerable<string>> CookieSignedWithDifferentSecurityKey()
    {
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = factory.CreateClient();
        using HttpResponseMessage response = await client.Login(UserName, Password);
        return response.GetCookies();
    }

    private WebApplicationFactory<Program> CreateWebApplicationFactory() =>
        Util.CreateWebApplicationFactory(_fileStoreDirectory);

    private static Task<HttpResponseMessage> CallApi(HttpClient client) => client.GetAsync("/Cards/Next");
}