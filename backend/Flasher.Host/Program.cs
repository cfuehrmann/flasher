using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;

using Flasher.Host;
using Flasher.Injectables;
using Flasher.Store.Cards;
using Flasher.Store.Exceptions;
using Flasher.Store.FileStore.Cards;

using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Authorization;
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

var builder = WebApplication.CreateBuilder(args);

builder.Host
    .ConfigureAppConfiguration((hostingContext, config) =>
        config.AddEnvironmentVariables(prefix: "Flasher_"))
    .ConfigureLogging(logging =>
        logging.ClearProviders().AddConsole().SetMinimumLevel(LogLevel.Information));

var services = builder.Services;

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
    .AddAuthorization(options =>
        options.AddPolicy("Bearer",
            new AuthorizationPolicyBuilder(JwtBearerDefaults.AuthenticationScheme)
                .RequireAuthenticatedUser()
                .Build()))
    .AddSwaggerGen(c =>
        c.SwaggerDoc("v1", new OpenApiInfo { Title = "Flasher API", Version = "v1" }))
    .AddScoped<IPasswordHasher<User>, PasswordHasher<User>>()
    .AddSingleton<IDateTime, SystemDateTime>()
    .AddSingleton(_ => RSA.Create())
    .AddSingleton<SecurityKey>(serviceProvider => new RsaSecurityKey(serviceProvider.GetRequiredService<RSA>()));

var hostAssembly = Assembly.GetExecutingAssembly();
var storeAssembly = typeof(ICardStore).Assembly;
var fileStoreAssembly = typeof(CardStore).Assembly;
var hostTypes = hostAssembly.GetExportedTypes();
var storeTypes = storeAssembly.GetExportedTypes();
var fileStoreTypes = fileStoreAssembly.GetExportedTypes();
AddSingletons(storeTypes, fileStoreTypes);
AddSingletons(fileStoreTypes, fileStoreTypes);
ConfigureServices(hostTypes);
ConfigureServices(fileStoreTypes);

var app = builder.Build();

app
    .UseExceptionHandler(errorApp =>
        errorApp.Run(async context =>
            {
                context.Response.ContentType = "text/html";
                var error = context.Features.Get<IExceptionHandlerPathFeature>()?.Error;
                if (error is ConflictException)
                {
                    context.Response.StatusCode = StatusCodes.Status409Conflict;
                    var text = error.Message ?? "Conflict while accessing a file!";
                    await context.Response.WriteAsync(text);
                    return;
                }
                context.Response.StatusCode = StatusCodes.Status500InternalServerError;
                await context.Response.WriteAsync("Internal server error!");
            }))
    .UseSwagger()
    .UseSwaggerUI(c => c.SwaggerEndpoint("/swagger/v1/swagger.json", "Flasher API"))
    .UseRouting()
    .Use(async (context, next) =>
        {
            if (context.Request.Cookies.TryGetValue("__Host-jwt", out var value))
                context.Request.Headers.Append("Authorization", "Bearer " + value);
            await next.Invoke();
        })
    .UseAuthentication()
    .UseAuthorization();

app.MapControllers();

app.Run();

void AddSingletons(IEnumerable<Type> interfaceAssemblyTypes, IEnumerable<Type> implementationAssemblyTypes)
{
    var registrations =
        from interfaceType in interfaceAssemblyTypes
        where interfaceType.IsInterface
        let implementations =
            from type in implementationAssemblyTypes
            where type.GetInterfaces().Contains(interfaceType)
            select type
        select (interfaceType, implementations);

    foreach (var (interfaceType, implementations) in registrations)
    {
        var count = implementations.Count();

        if (count != 1)
            throw new InvalidOperationException
                ($"There are {count} implementations for {interfaceType.Name}, but exactly one is needed!");

        _ = services.AddSingleton(interfaceType, implementations.First());
    }
}

void ConfigureServices(IEnumerable<Type> optionCandidates)
{
    var optionTypes =
        from type in optionCandidates
        where type.Name.EndsWith("Options", StringComparison.Ordinal)
        let prefix = type.Name[..^7]
        select (type, prefix);

    var configure =
        typeof(OptionsConfigurationServiceCollectionExtensions)
            .GetMethods()
            .Single(method => method.GetParameters().Length == 2);

    foreach (var (tOptions, prefix) in optionTypes)
        _ = configure
            .MakeGenericMethod(tOptions)
            .Invoke(null, new object[] { services, builder.Configuration.GetSection(prefix) });
}

#pragma warning disable CA1050 
public partial class Program { } // Make public for integration tests, less troublesome than InternalsVisibleTo
#pragma warning restore CA1050


