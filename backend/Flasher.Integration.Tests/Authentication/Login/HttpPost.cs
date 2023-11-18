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
        using WebApplicationFactory<Program> factory =
            new WebApplicationFactory<Program>().WithWebHostBuilder(
                builder =>
                    builder.ConfigureServices(
                        services =>
                            services
                                .Configure(
                                    (Action<AuthenticationOptions>)(
                                        option =>
                                            option.TokenLifetime = TimeSpan.FromSeconds(
                                                tokenLifetime
                                            )
                                    )
                                )
                                .Configure(
                                    (Action<FileStoreOptions>)(
                                        option => option.Directory = _fileStoreDirectory
                                    )
                                )
                    )
            );
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage loginResponse = await client.Login(UserName, Password);
        IEnumerable<string> cookies = loginResponse.GetCookies();
        client.AddCookies(cookies);
        using HttpResponseMessage apiResponse = await CallApi(client);

        Assert.Equal(HttpStatusCode.OK, loginResponse.StatusCode);
        string? jwtCookie = cookies.FirstOrDefault(
            cookie => cookie.StartsWith("__Host-jwt", StringComparison.Ordinal)
        );
        Assert.NotNull(jwtCookie);
        Assert.Contains("; Path=/", jwtCookie, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("; Secure", jwtCookie, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("; HttpOnly", jwtCookie, StringComparison.OrdinalIgnoreCase);
        Assert.Contains(" SameSite=strict", jwtCookie, StringComparison.OrdinalIgnoreCase);
        Assert.Contains(
            $"; Max-Age={tokenLifetime}",
            jwtCookie,
            StringComparison.OrdinalIgnoreCase
        );
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
        using WebApplicationFactory<Program> factory =
            new WebApplicationFactory<Program>().WithWebHostBuilder(
                builder =>
                    builder.ConfigureServices(
                        services =>
                            services.Configure<FileStoreOptions>(option => option.Directory = null)
                    )
            );
        using HttpClient client = factory.CreateClient();

        using HttpResponseMessage response = await client.Login(UserName, Password);

        Assert.Equal(HttpStatusCode.InternalServerError, response.StatusCode);
        Assert.Equal(MediaTypeHeaderValue.Parse("text/html"), response.Content.Headers.ContentType);
        string content = await response.Content.ReadAsStringAsync();
        Assert.Contains("store", content, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("configuration", content, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("filestore", content, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("directory", content, StringComparison.OrdinalIgnoreCase);
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
        Assert.Contains("file", content, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("invalid", content, StringComparison.OrdinalIgnoreCase);
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

        string? jwtCookie = cookies.FirstOrDefault(
            cookie => cookie.StartsWith("__Host-jwt", StringComparison.Ordinal)
        );
        // Check that the token's Max-Age is not 0. Because 0 corresponds to the default
        // TimeSpan, which would be used if the property were not explicitly configured.
        // We don't suppress the hint below for now, because the best fix may be to
        // heed the hint. That's just not possible right now because it would require
        // the test class to be partial, which the test runner currently can't deal with.
        Assert.Matches(new Regex("; Max-Age=[1..9]", RegexOptions.IgnoreCase), jwtCookie);
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
