using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;

namespace Flasher.Integration.Tests.Cards;

public sealed class HttpGet : IDisposable
{
    private const string Id0 = "0d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id1 = "1d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id2 = "2d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id3 = "3d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id4 = "4d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id5 = "5d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id6 = "6d305cfd-9a33-46cd-807b-8adefbe57e42";
    private const string Id7 = "7d305cfd-9a33-46cd-807b-8adefbe57e42";

    private const string UserName = "john@doe";
    private const string Password = "123456";

    // The hash comes from the password. Don't let this test suite compute the hash, because
    // these tests should also protect against invalidating the password hash by accidental
    // change of the hash algorithm.
    private const string PasswordHash =
        "AQAAAAIAAYagAAAAENaCGNNEyy7NIj6ytU5fjbj4ze0Rs10SHU3WAaX+Fw1EV3mix/ytgxvbp7JMVYAsoQ==";

    private readonly string _fileStoreDirectory;

    public HttpGet()
    {
        _fileStoreDirectory = Util.InventFileStoreDirectory();
        Util.CreateUserStore(_fileStoreDirectory, UserName, PasswordHash);
    }

    public void Dispose()
    {
        Directory.Delete(_fileStoreDirectory, true);
    }

    [Fact]
    public async Task NoSearchTextReturnsAll()
    {
        var cardStrings0 = new CardStrings(
            Id0,
            "Prompt0",
            "Solution0",
            "New",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643460+02:00",
            "false"
        );
        var cardStrings1 = new CardStrings(
            Id1,
            "Prompt1",
            "Solution1",
            "Ok",
            "2022-04-02T15:01:13.1643461+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "true"
        );
        var cardStrings2 = new CardStrings(
            Id2,
            "Prompt2",
            "Solution2",
            "Failed",
            "2022-04-02T15:01:13.1643462+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "true"
        );
        WriteCardsFile(cardStrings1, cardStrings0, cardStrings2);
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards");

        _ = await Verify(
            new
            {
                ExpectedCards = new[]
                {
                    cardStrings0.FullCard,
                    cardStrings1.FullCard,
                    cardStrings2.FullCard,
                },
                response,
            }
        );
    }

    [Fact]
    public async Task SearchingPrompt()
    {
        var cardStrings0 = new CardStrings(
            Id0,
            "different",
            "Solution0",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643460+02:00",
            "true"
        );
        var cardStrings1 = new CardStrings(
            Id1,
            "Prompt1",
            "Solution1",
            "Failed",
            "2022-04-02T15:01:13.1643461+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "true"
        );
        var cardStrings2 = new CardStrings(
            Id2,
            "different",
            "Solution2",
            "Failed",
            "2022-04-02T15:01:13.1643462+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "true"
        );
        WriteCardsFile(cardStrings1, cardStrings0, cardStrings2);
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards?searchText=PrOmPt");

        _ = await Verify(new { ExpectedCard = cardStrings1.FullCard, response });
    }

    [Fact]
    public async Task SearchingSolution()
    {
        var cardStrings0 = new CardStrings(
            Id0,
            "Prompt0",
            "different",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643460+02:00",
            "true"
        );
        var cardStrings1 = new CardStrings(
            Id1,
            "Prompt1",
            "Solution1",
            "Failed",
            "2022-04-02T15:01:13.1643461+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "true"
        );
        var cardStrings2 = new CardStrings(
            Id2,
            "Prompt2",
            "different",
            "Failed",
            "2022-04-02T15:01:13.1643462+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "true"
        );
        WriteCardsFile(cardStrings0, cardStrings1, cardStrings2);
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards?searchText=SoLuTiOn");

        _ = await Verify(new { ExpectedCard = cardStrings1.FullCard, response });
    }

    [Fact]
    public async Task FileEmpty()
    {
        WriteCardsFile();
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards");

        _ = await Verify(new { response });
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task EnabledCardsFirst(bool reverse)
    {
        var enabledCardStrings = new CardStrings(
            Id1,
            "Prompt",
            "Solution",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "false"
        );
        var disabledCardStrings = new CardStrings(
            Id0,
            "Prompt",
            "Solution",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "true"
        );
        var cardsStrings = Util.Reverse(reverse, enabledCardStrings, disabledCardStrings);
        WriteCardsFile(cardsStrings);
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards");

        _ = await Verify(
                new
                {
                    ExpectedCards = new[]
                    {
                        enabledCardStrings.FullCard,
                        disabledCardStrings.FullCard,
                    },
                    response,
                }
            )
            .UseParameters(reverse);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task EarlyNextTimeFirst(bool reverse)
    {
        var earlyNextTimeCardStrings = new CardStrings(
            Id1,
            "Prompt",
            "Solution",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "false"
        );
        var lateNextTimeCardStrings = new CardStrings(
            Id0,
            "Prompt",
            "Solution",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "false"
        );
        var cardsStrings = Util.Reverse(reverse, earlyNextTimeCardStrings, lateNextTimeCardStrings);
        WriteCardsFile(cardsStrings);
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards");

        _ = await Verify(
                new
                {
                    ExpectedCards = new Dictionary<string, object>
                    {
                        { "early", earlyNextTimeCardStrings.FullCard },
                        { "late", lateNextTimeCardStrings.FullCard },
                    },
                    response,
                }
            )
            .UseParameters(reverse);
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    public async Task WithSkip(int skip)
    {
        var cardStrings0 = new CardStrings(
            Id0,
            "different",
            "Solution0",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643460+02:00",
            "true"
        );
        var cardStrings1 = new CardStrings(
            Id1,
            "Prompt1",
            "Solution1",
            "Failed",
            "2022-04-02T15:01:13.1643461+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "false"
        );
        var cardStrings2 = new CardStrings(
            Id2,
            "different",
            "Solution2",
            "Failed",
            "2022-04-02T15:01:13.1643462+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "true"
        );
        var cardStrings3 = new CardStrings(
            Id3,
            "Prompt3",
            "Solution3",
            "Ok",
            "2022-04-02T15:01:13.1643463+02:00",
            "2022-04-10T15:31:13.1643463+02:00",
            "true"
        );
        WriteCardsFile(cardStrings0, cardStrings1, cardStrings2, cardStrings3);
        using var factory = GetApplicationFactory();
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards?searchText=PrOmPt&skip={skip}");

        _ = await Verify(
                new
                {
                    ExpectedCards = new[] { cardStrings1.FullCard, cardStrings3.FullCard }.Skip(
                        skip
                    ),
                    response,
                }
            )
            .UseParameters(skip);
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    [InlineData(4)]
    public async Task PageSize(int pageSize)
    {
        var c0 = new CardStrings(
            Id0,
            "different",
            "Solution0",
            "Failed",
            "2022-04-02T15:01:13.1643460+02:00",
            "2022-04-10T15:31:13.1643460+02:00",
            "true"
        );
        var c1 = new CardStrings(
            Id1,
            "Prompt1",
            "Solution1",
            "Failed",
            "2022-04-02T15:01:13.1643461+02:00",
            "2022-04-10T15:31:13.1643461+02:00",
            "false"
        );
        var c2 = new CardStrings(
            Id2,
            "different",
            "Solution2",
            "Failed",
            "2022-04-02T15:01:13.1643462+02:00",
            "2022-04-10T15:31:13.1643462+02:00",
            "true"
        );
        var c3 = new CardStrings(
            Id3,
            "Prompt3",
            "Solution3",
            "Ok",
            "2022-04-02T15:01:13.1643463+02:00",
            "2022-04-10T15:31:13.1643463+02:00",
            "true"
        );
        var c4 = new CardStrings(
            Id4,
            "different",
            "Solution4",
            "Ok",
            "2022-04-02T15:01:13.1643464+02:00",
            "2022-04-10T15:31:13.1643464+02:00",
            "true"
        );
        var c5 = new CardStrings(
            Id5,
            "Prompt5",
            "Solution5",
            "Ok",
            "2022-04-02T15:01:13.1643465+02:00",
            "2022-04-10T15:31:13.1643465+02:00",
            "true"
        );
        var c6 = new CardStrings(
            Id6,
            "different",
            "Solution6",
            "Ok",
            "2022-04-02T15:01:13.1643466+02:00",
            "2022-04-10T15:31:13.1643466+02:00",
            "true"
        );
        var c7 = new CardStrings(
            Id7,
            "Prompt7",
            "Solution7",
            "Ok",
            "2022-04-02T15:01:13.1643467+02:00",
            "2022-04-10T15:31:13.1643467+02:00",
            "true"
        );
        WriteCardsFile(c1, c5, c0, c3, c2, c4, c7, c6);
        using var factory = GetApplicationFactory(pageSize);
        using var client = await Login(factory);

        using var response = await client.GetAsync($"/Cards?searchText=PrOmPt&skip=1");

        _ = await Verify(
                new
                {
                    ExpectedCards = new[] { c3.FullCard, c5.FullCard, c7.FullCard }.Take(pageSize),
                    response,
                }
            )
            .UseParameters(pageSize);
    }

    private WebApplicationFactory<Program> GetApplicationFactory(int pageSize = 99)
    {
        var settings = new Dictionary<string, string?>
        {
            { "FileStore:Directory", _fileStoreDirectory },
            { "Cards:PageSize", $"{pageSize}" },
        };
        var factory = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
            builder.ConfigureAppConfiguration(
                (context, conf) =>
                {
                    _ = conf.AddInMemoryCollection(settings);
                }
            )
        );
        return factory;
    }

    private void WriteCardsFile(params CardStrings[] cardsStrings)
    {
        Util.WriteCardsFile(_fileStoreDirectory, UserName, from c in cardsStrings select c.Json);
    }

    private void WriteCardsFile(IEnumerable<CardStrings> cardsStrings)
    {
        Util.WriteCardsFile(_fileStoreDirectory, UserName, from c in cardsStrings select c.Json);
    }

    private static async Task<HttpClient> Login(WebApplicationFactory<Program> factory)
    {
        return await factory.Login(UserName, Password);
    }
}
