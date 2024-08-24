using System.Security.Cryptography;
using System.Text.Json.Serialization;
using Flasher.Host;
using Flasher.Host.Handlers.Authentication;
using Flasher.Host.Handlers.AutoSaving;
using Flasher.Host.Handlers.Cards;
using Flasher.Host.Handlers.History;
using Flasher.Injectables;
using Flasher.Store.Authentication;
using Flasher.Store.AutoSaving;
using Flasher.Store.Cards;
using Flasher.Store.FileStore;
using Flasher.Store.FileStore.Authentication;
using Flasher.Store.FileStore.AutoSaving;
using Flasher.Store.FileStore.Cards;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Identity;
using Microsoft.AspNetCore.Routing.Constraints;
using Microsoft.IdentityModel.Tokens;

var builder = WebApplication.CreateSlimBuilder(args);

var services = builder.Services;

#if DEBUG
services.AddEndpointsApiExplorer().AddSwaggerGen();
#endif

services.Configure<RouteOptions>(static options =>
    options.SetParameterPolicy<RegexInlineRouteConstraint>("regex")
);

services.AddScoped<IPasswordHasher<User>, PasswordHasher<User>>();
services.AddSingleton<IAuthenticationStore, AuthenticationStore>();
services.AddSingleton<ICardStore, CardStore>();
services.AddSingleton<IAutoSaveStore, AutoSaveStore>();
services.AddSingleton<IFileStoreJsonContextProvider, FileStoreJsonContextProvider>();
services.Configure<FileStoreOptions>(builder.Configuration.GetSection("FileStore"));
services.Configure<CardsOptions>(builder.Configuration.GetSection("Cards"));

// The following method call is needed for AOT only. I see no way to cover it
// by automated tests.
services.ConfigureHttpJsonOptions(static options =>
{
    options.SerializerOptions.TypeInfoResolverChain.Insert(0, AppJsonSerializerContext.Default);
});

var authorizationBuilder = services.AddAuthorizationBuilder();

authorizationBuilder.AddFallbackPolicy(
    "", // The name matters nowhere, so it cannot be covered by tests.
    static policy =>
    {
        policy.RequireAuthenticatedUser();
    }
);

var authenticationBuilder = services.AddAuthentication("Bearer");
authenticationBuilder.AddJwtBearer();

services
    .AddOptions<JwtBearerOptions>(JwtBearerDefaults.AuthenticationScheme)
    .Configure<SecurityKey>(
        static (options, signingKey) =>
            options.TokenValidationParameters = new TokenValidationParameters
            {
                IssuerSigningKey = signingKey,
                ValidateAudience = false,
                ValidateIssuer = false,
            }
    );

services.AddSingleton(static _ => RSA.Create());

services.AddSingleton<SecurityKey>(static serviceProvider => new RsaSecurityKey(
    serviceProvider.GetRequiredService<RSA>()
));

services.Configure<AuthenticationOptions>(builder.Configuration.GetSection("Authentication"));

services.AddSingleton<IDateTime, SystemDateTime>();

var app = builder.Build();

#if DEBUG
_ = app.UseSwagger().UseSwaggerUI();
#endif

app.Use(
    static async (context, next) =>
    {
        if (context.Request.Cookies.TryGetValue("__Host-jwt", out string? value))
        {
            context.Request.Headers.Append("Authorization", "Bearer " + value);
        }

        await next.Invoke();
    }
);
app.UseAuthentication();
app.UseAuthorization();


{
    var group = app.MapGroup("/Authentication");
    var handler = group.MapPost("/Login", LoginHandler.Login);
    handler.AllowAnonymous();
}


{
    var group = app.MapGroup("/Cards");
    group.MapPost("", CardsHandler.Create);
    group.MapPatch("/{id}", CardsHandler.Update);
    group.MapDelete("/{id}", CardsHandler.Delete);
    group.MapGet("", CardsHandler.Find);
    group.MapGet("/Next", CardsHandler.Next);
    group.MapPost("/{id}/SetOk", CardsHandler.SetOk);
    group.MapPost("/{id}/SetFailed", CardsHandler.SetFailed);
    group.MapPost("/{id}/Enable", CardsHandler.Enable);
    group.MapPost("/{id}/Disable", CardsHandler.Disable);
}


{
    var group = app.MapGroup("/AutoSave");
    group.MapPut("", AutoSaveHandler.Write);
    group.MapDelete("", AutoSaveHandler.Delete);
}


{
    var group = app.MapGroup("/History");
    group.MapDelete("/{id}", HistoryHandler.Delete);
}

app.Run();

[JsonSourceGenerationOptions(UseStringEnumConverter = true)]
[JsonSerializable(typeof(LoginRequest))]
[JsonSerializable(typeof(LoginResponse))]
[JsonSerializable(typeof(FullCard))]
[JsonSerializable(typeof(FindResponse))]
[JsonSerializable(typeof(CreateCardRequest))]
[JsonSerializable(typeof(UpdateCardRequest))]
[JsonSerializable(typeof(WriteAutoSaveRequest))]
internal sealed partial class AppJsonSerializerContext : JsonSerializerContext { }

#pragma warning disable CA1050
// Make public for automated tests, less troublesome than InternalsVisibleTo
public partial class Program { }
#pragma warning restore CA1050
