using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;
using Flasher.Injectables;
using Flasher.Store.Cards;
using Flasher.Store.Exceptions;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Diagnostics;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Identity;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.IdentityModel.Tokens;
using Microsoft.OpenApi.Models;

[assembly: ApiController]

namespace Flasher.Host;

public class Startup
{
    public Startup(IConfiguration configuration) => Configuration = configuration;

    public IConfiguration Configuration { get; }

    // This method gets called by the runtime. Use this method to add services to the container.
    public void ConfigureServices(IServiceCollection services)
    {
        _ = services
                .AddControllers()
                .AddJsonOptions(options =>
                {
                    var item = new JsonStringEnumConverter(JsonNamingPolicy.CamelCase);
                    options.JsonSerializerOptions.Converters.Add(item);
                });

        var securityKey = new RsaSecurityKey(RSA.Create());

        _ = services
                .AddAuthentication(options =>
                    {
                        options.DefaultAuthenticateScheme = JwtBearerDefaults.AuthenticationScheme;
                        options.DefaultChallengeScheme = JwtBearerDefaults.AuthenticationScheme;
                    })
                .AddJwtBearer(options =>
                    {
                        options.TokenValidationParameters = new TokenValidationParameters
                        {
                            IssuerSigningKey = securityKey,
                            ValidateAudience = false,
                            ValidateIssuer = false
                        };
                    });

        _ = services
                .AddAuthorization(options =>
                    options.AddPolicy("Bearer",
                        new AuthorizationPolicyBuilder(JwtBearerDefaults.AuthenticationScheme)
                            .RequireAuthenticatedUser()
                            .Build()))
                .AddSwaggerGen(c =>
                    c.SwaggerDoc("v1", new OpenApiInfo { Title = "Flasher API", Version = "v1" }))
                .AddSingleton(securityKey)
                .AddScoped<IPasswordHasher<User>, PasswordHasher<User>>()
                .AddSingleton<IDateTime, SystemDateTime>();

        RegisterStoreByConvention(services);
    }

    private void RegisterStoreByConvention(IServiceCollection services)
    {
        var hostAssembly = Assembly.GetExecutingAssembly();
        var interfaceAssembly = typeof(ICardStore).Assembly;
        // The line below would need changing if we discovered the implementation assembly dynamically
        var implementationAssembly = typeof(Store.FileStore.Cards.CardStore).Assembly;
        var hostAssemblyTypes = hostAssembly.GetExportedTypes();
        var interfaceAssemblyTypes = interfaceAssembly.GetExportedTypes();
        var implementationAssemblyTypes = implementationAssembly.GetExportedTypes();
        RegisterStoreImplementations(services, interfaceAssemblyTypes, implementationAssemblyTypes);
        ConfigureOptionsByConvention(services, hostAssemblyTypes);
        ConfigureOptionsByConvention(services, implementationAssemblyTypes);
    }

    private static void RegisterStoreImplementations(IServiceCollection services,
        IEnumerable<Type> interfaceAssemblyTypes, IEnumerable<Type> implementationAssemblyTypes)
    {
        var registrations =
            from interfaceType in interfaceAssemblyTypes
            where
                interfaceType.Namespace != null &&
                interfaceType.Namespace.StartsWith("Flasher.Store", StringComparison.Ordinal) &&
                interfaceType.IsInterface
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

    private void ConfigureOptionsByConvention(IServiceCollection services, IEnumerable<Type> candidateTypes)
    {
        var optionTypes =
            from type in candidateTypes
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
                .Invoke(null, new object[] { services, Configuration.GetSection(prefix) });
    }

    // This method gets called by the runtime. Use this method to configure the HTTP request pipeline.
    public void Configure(IApplicationBuilder app)
    {
        _ = app
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
            .UseAuthorization()
            .UseEndpoints(endpoints => endpoints.MapControllers());
    }
}
