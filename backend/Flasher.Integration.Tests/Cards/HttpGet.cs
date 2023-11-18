using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Threading.Tasks;
using Flasher.Host;
using Flasher.Store.Cards;
using Flasher.Store.FileStore;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

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
    private const string PromptSubString = "PrOmPt";
    private static readonly string Prompt0 =
        $"foo{PromptSubString.ToUpper(CultureInfo.InvariantCulture)}bar";
    private const string SolutionSubstring = "SoLuTiOn";
    private static readonly string Solution0 =
        $"foo{SolutionSubstring.ToUpper(CultureInfo.InvariantCulture)}bar";
    private const string NewString = "New";
    private const string FailedString = "Failed";
    private const string OkString = "Ok";
    private const string Time0String = "2022-04-02T15:01:13.1643461+02:00";
    private const string Time1String = "2022-04-02T15:31:13.1643461+02:00";
    private const string Time2String = "2022-04-10T15:31:13.1643461+02:00";
    private const string FalseString = "false";
    private const string TrueString = "true";

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
            Prompt0,
            Solution0,
            NewString,
            Time0String,
            Time1String,
            FalseString
        );
        var cardStrings1 = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings2 = new CardStrings(
            Id2,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        WriteCardsFile(cardStrings0, cardStrings1, cardStrings2);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync($"/Cards");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        IEnumerable<FullCard> cards = body!.Cards;
        Assert.Equal(3, cards.Count());
        Assert.Contains(cardStrings0.FullCard, cards);
        Assert.Contains(cardStrings1.FullCard, cards);
        Assert.Contains(cardStrings2.FullCard, cards);
    }

    [Fact]
    public async Task SearchingPrompt()
    {
        var cardStrings0 = new CardStrings(
            Id0,
            "different",
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings1 = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings2 = new CardStrings(
            Id2,
            "different",
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        WriteCardsFile(cardStrings0, cardStrings1, cardStrings2);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync(
            $"/Cards?searchText={PromptSubString}"
        );

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(new[] { cardStrings1.FullCard }, body.Cards);
    }

    [Fact]
    public async Task SearchingSolution()
    {
        var cardStrings0 = new CardStrings(
            Id0,
            Prompt0,
            "different",
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings1 = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings2 = new CardStrings(
            Id2,
            Prompt0,
            "different",
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        WriteCardsFile(cardStrings0, cardStrings1, cardStrings2);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync(
            $"/Cards?searchText={SolutionSubstring}"
        );

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(new[] { cardStrings1.FullCard }, body.Cards);
    }

    [Fact]
    public async Task FileEmpty()
    {
        WriteCardsFile();
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync($"/Cards");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Empty(body.Cards);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task EnabledCardsFirst(bool reverse)
    {
        var enabledCardStrings = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time2String,
            FalseString
        );
        var disabledCardStrings = new CardStrings(
            Id0,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        IEnumerable<CardStrings> cardsStrings = Util.Reverse(
            reverse,
            enabledCardStrings,
            disabledCardStrings
        );
        WriteCardsFile(cardsStrings);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync($"/Cards");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(
            new[] { enabledCardStrings.FullCard, disabledCardStrings.FullCard },
            body.Cards
        );
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task EarlyNextTimeFirst(bool reverse)
    {
        var earlyNextTimeCardStrings = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            FalseString
        );
        var lateNextTimeCardStrings = new CardStrings(
            Id0,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time2String,
            FalseString
        );
        IEnumerable<CardStrings> cardsStrings = Util.Reverse(
            reverse,
            earlyNextTimeCardStrings,
            lateNextTimeCardStrings
        );
        WriteCardsFile(cardsStrings);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync($"/Cards");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(
            new[] { earlyNextTimeCardStrings.FullCard, lateNextTimeCardStrings.FullCard },
            body.Cards
        );
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task SmallIdFirst(bool reverse)
    {
        var smallIdCardStrings = new CardStrings(
            Id0,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            FalseString
        );
        var bigIdCardStrings = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            FalseString
        );
        IEnumerable<CardStrings> cardsStrings = Util.Reverse(
            reverse,
            smallIdCardStrings,
            bigIdCardStrings
        );
        WriteCardsFile(cardsStrings);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync($"/Cards");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(new[] { smallIdCardStrings.FullCard, bigIdCardStrings.FullCard }, body.Cards);
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
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings1 = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            FalseString
        );
        var cardStrings2 = new CardStrings(
            Id2,
            "different",
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var cardStrings3 = new CardStrings(
            Id3,
            Prompt0,
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        WriteCardsFile(cardStrings0, cardStrings1, cardStrings2, cardStrings3);
        using WebApplicationFactory<Program> factory = CreateWebApplicationFactory();
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync(
            $"/Cards?searchText={PromptSubString}&skip={skip}"
        );

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(new[] { cardStrings1.FullCard, cardStrings3.FullCard }.Skip(skip), body.Cards);
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
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var c1 = new CardStrings(
            Id1,
            Prompt0,
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            FalseString
        );
        var c2 = new CardStrings(
            Id2,
            "different",
            Solution0,
            FailedString,
            Time0String,
            Time1String,
            TrueString
        );
        var c3 = new CardStrings(
            Id3,
            Prompt0,
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        var c4 = new CardStrings(
            Id4,
            "different",
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        var c5 = new CardStrings(
            Id5,
            Prompt0,
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        var c6 = new CardStrings(
            Id6,
            "different",
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        var c7 = new CardStrings(
            Id7,
            Prompt0,
            Solution0,
            OkString,
            Time0String,
            Time1String,
            TrueString
        );
        WriteCardsFile(c0, c1, c2, c3, c4, c5, c6, c7);
        using WebApplicationFactory<Program> factory =
            new WebApplicationFactory<Program>().WithWebHostBuilder(
                builder =>
                    builder.ConfigureServices(
                        services =>
                            services
                                .Configure<CardsOptions>(option => option.PageSize = pageSize)
                                .Configure<FileStoreOptions>(
                                    option => option.Directory = _fileStoreDirectory
                                )
                    )
            );
        using HttpClient client = await Login(factory);

        using HttpResponseMessage response = await client.GetAsync(
            $"/Cards?searchText={PromptSubString}&skip=1"
        );

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        FindResponse? body = await response.ReadJsonAsync<FindResponse>();
        Assert.NotNull(body);
        Assert.Equal(new[] { c3.FullCard, c5.FullCard, c7.FullCard }.Take(pageSize), body.Cards);
    }

    private void WriteCardsFile(params CardStrings[] cardsStrings)
    {
        Util.WriteCardsFile(_fileStoreDirectory, UserName, from c in cardsStrings select c.Json);
    }

    private void WriteCardsFile(IEnumerable<CardStrings> cardsStrings)
    {
        Util.WriteCardsFile(_fileStoreDirectory, UserName, from c in cardsStrings select c.Json);
    }

    private WebApplicationFactory<Program> CreateWebApplicationFactory()
    {
        return Util.CreateWebApplicationFactory(_fileStoreDirectory);
    }

    private static async Task<HttpClient> Login(WebApplicationFactory<Program> factory)
    {
        return await factory.Login(UserName, Password);
    }
}
