using System;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Http.Json;
using System.Threading.Tasks;

using Flasher.Host.Model;

using Microsoft.AspNetCore.Mvc.Testing;

using Xunit;

namespace Flasher.Integration.Tests.Cards;

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

    [Fact]
    public async Task Create()
    {
        using WebApplicationFactory<Program> factory = Util.CreateWebApplicationFactory(_fileStoreDirectory);
        using HttpClient client = await factory.Login(UserName, Password);

        using HttpResponseMessage response = await client.PostAsJsonAsync("/Cards", new CreateCardRequest { Prompt = "p", Solution = "s" });

        Assert.Equal(HttpStatusCode.Created, response.StatusCode);
    }

    // [Fact]
    // public async Task CreateThenFind()
    // {
    //     // Arrange

    //     var client = new WebApplicationFactory<Program>().CreateClient();
    //     var loginResponse = await client.Login(Util.UserName, Util.Password);
    //     IEnumerable<string>? cookies = loginResponse.GetCookies();
    //     client.AddCookies(cookies);

    //     // Act

    //     var createResponse = await client.PostAsJsonAsync("/Cards", new CreateCardRequest { Prompt = "p", Solution = "s" });

    //     var client2 = new WebApplicationFactory<Program>().CreateClient();
    //     var loginResponse2 = await client2.Login(Util.UserName, Util.Password);
    //     IEnumerable<string>? cookies2 = loginResponse2.GetCookies();
    //     client2.AddCookies(cookies2);
    //     var findResponse = await client2.GetAsync("/Cards?skip=0&searchText=p");
    //     var findResponse2 = await client2.GetAsync("/Cards?skip=0&searchText=s");

    //     // Assert

    //     _ = createResponse.EnsureSuccessStatusCode();
    //     _ = findResponse.EnsureSuccessStatusCode();
    //     _ = findResponse2.EnsureSuccessStatusCode();

    //     var jsonOptions = new JsonSerializerOptions { WriteIndented = true, PropertyNameCaseInsensitive = true };
    //     jsonOptions.Converters.Add(new JsonStringEnumConverter());

    //     var findResponseBody = await findResponse.Content.ReadFromJsonAsync<FindResponse>(jsonOptions);
    //     Assert.NotNull(findResponseBody);
    //     _ = Assert.Single(findResponseBody?.Cards);
    //     Assert.NotNull(findResponseBody?.Cards);

    //     var findResponseBody2 = await findResponse2.Content.ReadFromJsonAsync<FindResponse>(jsonOptions);
    //     Assert.NotNull(findResponseBody2);
    //     _ = Assert.Single(findResponseBody2?.Cards);
    //     Assert.NotNull(findResponseBody?.Cards);
    // }
}