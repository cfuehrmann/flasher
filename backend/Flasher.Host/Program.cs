using System;
using System.Linq;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;

using Flasher.Host;
using Flasher.Injectables;
using Flasher.Store.Authentication;
using Flasher.Store.AutoSaving;
using Flasher.Store.Cards;
using Flasher.Store.Exceptions;

using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Diagnostics;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Identity;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.IdentityModel.Tokens;
using Microsoft.OpenApi.Models;

[assembly: ApiController]

WebApplicationBuilder builder = WebApplication.CreateBuilder(args);

builder.Configuration.AddEnvironmentVariables(prefix: "Flasher_");
builder.Logging.ClearProviders().AddConsole().SetMinimumLevel(LogLevel.Information);

IServiceCollection services = builder.Services;

services
    .AddControllers()
    .AddJsonOptions(options =>
    {
        var item = new JsonStringEnumConverter(JsonNamingPolicy.CamelCase);
        options.JsonSerializerOptions.Converters.Add(item);
    });

services
    .AddOptions<JwtBearerOptions>(JwtBearerDefaults.AuthenticationScheme)
    .Configure<SecurityKey>((options, signingKey) => options.TokenValidationParameters = new TokenValidationParameters
    {
        IssuerSigningKey = signingKey,
        ValidateAudience = false,
        ValidateIssuer = false
    });

services
    .AddAuthentication(options =>
        {
            options.DefaultAuthenticateScheme = JwtBearerDefaults.AuthenticationScheme;
            options.DefaultChallengeScheme = JwtBearerDefaults.AuthenticationScheme;
        })
    .AddJwtBearer();

services
    .AddSwaggerGen(c => c.SwaggerDoc("v1", new OpenApiInfo { Title = "Flasher API", Version = "v1" }))

    .AddSingleton<IDateTime, SystemDateTime>()

    .AddScoped<IPasswordHasher<User>, PasswordHasher<User>>()
    .AddSingleton(_ => RSA.Create())
    .AddSingleton<SecurityKey>(serviceProvider => new RsaSecurityKey(serviceProvider.GetRequiredService<RSA>()))

    .Configure<AuthenticationOptions>(builder.Configuration.GetSection("Authentication"))
    .Configure<CardsOptions>(builder.Configuration.GetSection("Cards"))

    .AddSingleton<IAuthenticationStore, Flasher.Store.FileStore.Authentication.AuthenticationStore>()
    .AddSingleton<IAutoSaveStore, Flasher.Store.FileStore.AutoSaving.AutoSaveStore>()
    .AddSingleton<ICardStore, Flasher.Store.FileStore.Cards.CardStore>()
    .AddSingleton<Flasher.Store.FileStore.IFileStoreJsonContextProvider, Flasher.Store.FileStore.FileStoreJsonContextProvider>()
    .Configure<Flasher.Store.FileStore.FileStoreOptions>(builder.Configuration.GetSection("FileStore"));

WebApplication app = builder.Build();

app
    .UseExceptionHandler(errorApp =>
        errorApp.Run(async context =>
            {
                context.Response.ContentType = "text/html";
                Exception? error = context.Features.Get<IExceptionHandlerPathFeature>()?.Error;
                (int code, string text) = GetErrorDescription(error);
                context.Response.StatusCode = code;
                await context.Response.WriteAsync(text);
            }))
    .UseSwagger()
    .UseSwaggerUI(c => c.SwaggerEndpoint("/swagger/v1/swagger.json", "Flasher API"))
    .UseRouting()
    .Use(async (context, next) =>
        {
            if (context.Request.Cookies.TryGetValue("__Host-jwt", out string? value))
            {
                context.Request.Headers.Append("Authorization", "Bearer " + value);
            }

            await next.Invoke();
        })
    .UseAuthentication()
    .UseAuthorization();

app.MapControllers();

app.Run();

static (int, string) GetErrorDescription(Exception? error)
{
    return
        error is ConflictException
        ? (StatusCodes.Status409Conflict, error.Message)
        : error is StoreConfigurationException
        ? (StatusCodes.Status500InternalServerError, $"Problem with the configuration of the store: {error.Message}!")
        : (StatusCodes.Status500InternalServerError, error?.Message ?? "Unknown error!");
}

#pragma warning disable CA1050 
public partial class Program { } // Make public for integration tests, less troublesome than InternalsVisibleTo
#pragma warning restore CA1050
