using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using Flasher.Injectables;
using Flasher.Store.Authentication;
using Flasher.Store.AutoSaving;
using Microsoft.AspNetCore.Http.HttpResults;
using Microsoft.AspNetCore.Identity;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

namespace Flasher.Host.AOT.Handlers.Authentication;

public sealed class LoginHandler(
    IAuthenticationStore store,
    IAutoSaveStore autoSaveStore,
    IPasswordHasher<User> passwordHasher,
    IDateTime dateTime,
    IOptionsMonitor<AuthenticationOptions> options,
    SecurityKey securityKey
)
{
    public async Task<Results<Ok<LoginResponse>, UnauthorizedHttpResult>> Login(
        HttpContext httpContext,
        LoginRequest request
    )
    {
        string? hashedPassword = await store.GetPasswordHash(request.UserName);
        if (hashedPassword == null)
        {
            return TypedResults.Unauthorized();
        }

        PasswordVerificationResult passwordVerificationResult = passwordHasher.VerifyHashedPassword(
            new User { Name = request.UserName },
            hashedPassword,
            request.Password
        );

        if (passwordVerificationResult != PasswordVerificationResult.Success)
        {
            return TypedResults.Unauthorized();
        }

        Task<AutoSave?> readAutoSave = autoSaveStore.Read(request.UserName);

        string tokenString = GetTokenString(request.UserName);

        var cookieOptions = new CookieOptions
        {
            SameSite = SameSiteMode.Strict,
            Secure = true,
            HttpOnly = true,
            Path = "/",
            MaxAge = options.CurrentValue.TokenLifetime,
        };

        httpContext.Response.Cookies.Append("__Host-jwt", tokenString, cookieOptions);

        AutoSave? storedAutoSave = await readAutoSave;

        LoginResponse.AutoSaveData? autoSaveData =
            storedAutoSave != null
                ? new LoginResponse.AutoSaveData
                {
                    Id = storedAutoSave.Id,
                    Prompt = storedAutoSave.Prompt,
                    Solution = storedAutoSave.Solution,
                }
                : null;

        var response = new LoginResponse { JsonWebToken = tokenString, AutoSave = autoSaveData };

        return TypedResults.Ok(response);
    }

    private string GetTokenString(string userName)
    {
        var token = new JwtSecurityToken(
            claims: new[] { new Claim(ClaimTypes.Name, userName) },
            expires: dateTime.Now + options.CurrentValue.TokenLifetime,
            signingCredentials: new SigningCredentials(securityKey, SecurityAlgorithms.RsaSha256)
        );
        return new JwtSecurityTokenHandler().WriteToken(token);
    }
}
