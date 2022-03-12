using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.RegularExpressions;
using System.Threading.Tasks;

using Flasher.Host;
using Flasher.Host.Model;
using Flasher.Store.FileStore;

using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;

using Xunit;

namespace Flasher.Integration.Tests;

public sealed class LoginTests : IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";
    private const string FileStoreDirectoryEnvironmentVariable = "Flasher_FileStore__Directory";

    private readonly WebApplicationFactory<Program> _factory;
    private readonly string? _fileStoreDirectoryBeforeTest;

    public LoginTests()
    {
        _factory = new WebApplicationFactory<Program>();
        _fileStoreDirectoryBeforeTest = Environment.GetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable);
        Environment.SetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable, Directory.GetCurrentDirectory());
        CreateDatabase();
    }

    public void Dispose()
    {
        Environment.SetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable, _fileStoreDirectoryBeforeTest);
    }

    [Theory]
    [InlineData(42)]
    [InlineData(666)]
    public async Task LoginWithExistingUser(int tokenLifetime)
    {
        var client = _factory
         .WithWebHostBuilder(builder =>
             builder.ConfigureServices(services =>
                 services.Configure<AuthenticationOptions>(option =>
                    option.TokenLifetime = TimeSpan.FromSeconds(tokenLifetime))))
         .CreateClient();

        var loginResponse = await client.Login(UserName, Password);
        IEnumerable<string>? cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);
        var apiResponse = await client.CallApi();

        Assert.Equal(HttpStatusCode.OK, loginResponse.StatusCode);
        string? jwtCookie = cookies.FirstOrDefault(cookie =>
            cookie.StartsWith("__Host-jwt", StringComparison.Ordinal));
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
        var client = _factory.CreateClient();

        IEnumerable<string> cookies = await CookieSignedWithDifferentSecurityKey();
        client.AddCookies(cookies);
        var response = await client.CallApi();

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task LoginWithNonExistingUser()
    {
        var client = _factory.CreateClient();

        var response = await client.Login("jane@doe", Password);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task LoginWithWrongPassword()
    {
        var client = _factory.CreateClient();

        var response = await client.Login(UserName, "123457");

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task MissingDirectoryOption()
    {
        var client = _factory
            .WithWebHostBuilder(builder =>
                builder.ConfigureServices(services =>
                    services.Configure<FileStoreOptions>(option => option.Directory = null)))
            .CreateClient();

        var response = await client.Login(UserName, Password);

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
        var client = _factory.CreateClient();
        var usersPath = Path.Combine(FileStoreDirectory, "users.json");
        File.WriteAllText(usersPath, "null");

        var response = await client.Login(UserName, Password);

        Assert.Equal(HttpStatusCode.InternalServerError, response.StatusCode);
        string content = await response.Content.ReadAsStringAsync();
        Assert.Matches(new Regex("users", RegexOptions.IgnoreCase), content);
        Assert.Matches(new Regex("dictionary", RegexOptions.IgnoreCase), content);
        Assert.EndsWith("!", content);
        Assert.False(response.Headers.Contains("Set-Cookie"));
    }

    [Fact]
    public async Task OptionsAreInjected()
    {
        var client = _factory.CreateClient();

        var loginResponse = await client.Login(UserName, Password);
        IEnumerable<string>? cookies = loginResponse.GetCookies();

        string? jwtCookie = cookies.FirstOrDefault(cookie =>
            cookie.StartsWith("__Host-jwt", StringComparison.Ordinal));
        // Check that the token's Max-Age is not the default TimeSpan
        Assert.Matches(new Regex("; Max-Age=.*[1..9].*", RegexOptions.IgnoreCase), jwtCookie);
    }

    private static void CreateDatabase()
    {
        var usersPath = Path.Combine(FileStoreDirectory, "users.json");

        // The hash comes from the password. Don't let this test suite compute the hash, because
        // these tests should also protect against invalidating the user table by accidental
        // change of the hash algorithm.    
        var usersFileContent = $"{{ \"{UserName}\": \"AQAAAAEAACcQAAAAECUeTNmWxlWlEkteOikXzkwBM4VBrTYekVb9U+QBZjbcuk9V5ThbD4BfYDjzokwbVQ==\" }}";

        File.WriteAllText(usersPath, usersFileContent);
        string userPath = Path.Combine(FileStoreDirectory, UserName);
        _ = Directory.CreateDirectory(userPath);
        string cardsPath = Path.Combine(userPath, "cards.json");
        File.WriteAllText(cardsPath, "[]");
    }

    private static async Task<IEnumerable<string>> CookieSignedWithDifferentSecurityKey()
    {
        var client = new WebApplicationFactory<Program>().CreateClient();
        var response = await client.Login(UserName, Password);
        return response.GetCookies();
    }

    private static string FileStoreDirectory =>
        Environment.GetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable) ?? throw new InvalidOperationException();
}

internal static class LoginTestExtensions
{
    public static async Task<HttpResponseMessage> Login(this HttpClient client, string userName, string password) =>
        await client.PostAsJsonAsync("/Authentication/Login", new LoginRequest { UserName = userName, Password = password });

    public static IEnumerable<string> GetCookies(this HttpResponseMessage response) =>
        response.Headers.GetValues("Set-Cookie");

    public static void AddCookies(this HttpClient client, IEnumerable<string> cookies) =>
        client.DefaultRequestHeaders.Add("Cookie", cookies);

    public static Task<HttpResponseMessage> CallApi(this HttpClient client) =>
        client.GetAsync("/Cards/Next");
}