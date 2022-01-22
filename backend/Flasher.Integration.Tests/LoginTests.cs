using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Http.Json;
using System.Threading.Tasks;

using Flasher.Host.Model;

using Microsoft.AspNetCore.Mvc.Testing;

using Xunit;

namespace Flasher.Integration.Tests;

public sealed class LoginTests : IClassFixture<WebApplicationFactory<Program>>, IDisposable
{
    private const string UserName = "john@doe";
    private const string Password = "123456";
    private const string FileStoreDirectoryEnvironmentVariable = "Flasher_FileStore__Directory";

    private readonly WebApplicationFactory<Program> _factory;
    private readonly string? _fileStoreDirectoryBeforeTest;

    public LoginTests(WebApplicationFactory<Program> factory)
    {
        _factory = factory;
        _fileStoreDirectoryBeforeTest = Environment.GetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable);
        Environment.SetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable, Directory.GetCurrentDirectory());
        CreateDatabase();
    }

    public void Dispose()
    {
        Environment.SetEnvironmentVariable(FileStoreDirectoryEnvironmentVariable, _fileStoreDirectoryBeforeTest);
    }

    [Fact]
    public async Task LoginWithExistingUser()
    {
        var client = _factory.CreateClient();

        var loginResponse = await client.Login(UserName, Password);
        client.AddCookies(loginResponse.GetCookies());
        var apiResponse = await client.CallApi();

        Assert.Equal(HttpStatusCode.OK, loginResponse.StatusCode);
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