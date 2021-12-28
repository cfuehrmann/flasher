using System.Net;
using System.Net.Http.Json;
using System.Threading.Tasks;
using Flasher.Host.Model;
using Microsoft.AspNetCore.Mvc.Testing;
using Xunit;

namespace Flasher.Integration.Tests;

public class LoginTests : IClassFixture<WebApplicationFactory<Program>>
{
    private readonly WebApplicationFactory<Program> _factory;
    public LoginTests(WebApplicationFactory<Program> factory)
    {
        _factory = factory;
    }

    [Fact]
    public async Task LoginWithNonExistingUser()
    {
        // Arrange
        var client = _factory.CreateClient();

        // Act
        var request = new LoginRequest { UserName = "userThatMustNotExist", Password = "123456" };
        var response = await client.PostAsJsonAsync("/Authentication/Login", request);

        // Assert
        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }
}