#pragma warning disable CA1050

using System.Security.Cryptography;
using System.Text.Json.Serialization;
using Flasher.Host.AOT;
using Flasher.Host.AOT.Handlers.Authentication;
using Flasher.Host.AOT.Handlers.Cards;
using Flasher.Injectables;
using Flasher.Store.Authentication;
using Flasher.Store.AutoSaving;
using Flasher.Store.Cards;
using Flasher.Store.FileStore.AutoSaving;
using Flasher.Store.FileStore.Cards;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Http.HttpResults;
using Microsoft.AspNetCore.Identity;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

var builder = WebApplication.CreateSlimBuilder(args);

builder.Configuration.AddEnvironmentVariables(prefix: "Flasher_");
builder.Logging.ClearProviders().AddConsole().SetMinimumLevel(LogLevel.Information);

var services = builder.Services;

#if DEBUG
services.AddEndpointsApiExplorer().AddSwaggerGen();
#endif

services
    .AddScoped<IPasswordHasher<User>, PasswordHasher<User>>()
    .AddSingleton<
        IAuthenticationStore,
        Flasher.Store.FileStore.Authentication.AuthenticationStore
    >()
    .AddSingleton<ICardStore, CardStore>()
    .AddSingleton<IAutoSaveStore, AutoSaveStore>()
    .AddSingleton<
        Flasher.Store.FileStore.IFileStoreJsonContextProvider,
        Flasher.Store.FileStore.FileStoreJsonContextProvider
    >()
    .Configure<Flasher.Store.FileStore.FileStoreOptions>(
        builder.Configuration.GetSection("FileStore")
    )
    .Configure<CardsOptions>(builder.Configuration.GetSection("Cards"));

services.ConfigureHttpJsonOptions(options =>
{
    options.SerializerOptions.TypeInfoResolverChain.Insert(0, AppJsonSerializerContext.Default);
});

services.AddAuthorization();

services.AddAuthentication("Bearer").AddJwtBearer();

services
    .AddOptions<JwtBearerOptions>(JwtBearerDefaults.AuthenticationScheme)
    .Configure<SecurityKey>(
        (options, signingKey) =>
            options.TokenValidationParameters = new TokenValidationParameters
            {
                IssuerSigningKey = signingKey,
                ValidateAudience = false,
                ValidateIssuer = false
            }
    );

services
    .AddSingleton(_ => RSA.Create())
    .AddSingleton<SecurityKey>(
        serviceProvider => new RsaSecurityKey(serviceProvider.GetRequiredService<RSA>())
    );

services.Configure<AuthenticationOptions>(builder.Configuration.GetSection("Authentication"));

services.AddSingleton<IDateTime, SystemDateTime>();

services.AddScoped<LoginHandler, LoginHandler>();

var app = builder.Build();

#if DEBUG
_ = app.UseSwagger().UseSwaggerUI();
#endif

app.Use(
        async (context, next) =>
        {
            if (context.Request.Cookies.TryGetValue("__Host-jwt", out string? value))
            {
                context.Request.Headers.Append("Authorization", "Bearer " + value);
            }

            await next.Invoke();
        }
    )
    .UseAuthentication()
    .UseAuthorization();

var authenticationGroup = app.MapGroup("/Authentication");
authenticationGroup
    .MapPost(
        "/Login",
        async Task<Results<Ok<LoginResponse>, UnauthorizedHttpResult>> (
            HttpContext context,
            LoginRequest request,
            LoginHandler handler
        ) => await handler.Login(context, request)
    )
    .Produces(StatusCodes.Status401Unauthorized);

var cardsGroup = app.MapGroup("/Cards").RequireAuthorization();

cardsGroup.MapGet("", CardsHandler.Find);
cardsGroup.MapGet("Next", CardsHandler.Next);

app.Run();

[JsonSerializable(typeof(LoginRequest))]
[JsonSerializable(typeof(LoginResponse))]
internal sealed partial class AppJsonSerializerContext : JsonSerializerContext { }

#pragma warning disable CA1050
public partial class Program { } // Make public for integration tests, less troublesome than InternalsVisibleTo
#pragma warning restore CA1050
