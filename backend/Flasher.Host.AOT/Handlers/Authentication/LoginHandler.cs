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

public static class LoginHandler
{
    public static async Task<Results<Ok<LoginResponse>, UnauthorizedHttpResult>> Login(
        LoginRequest request,
        HttpContext context,
        IAuthenticationStore authenticationStore,
        IAutoSaveStore autoSaveStore,
        IPasswordHasher<User> passwordHasher,
        IDateTime dateTime,
        IOptionsMonitor<AuthenticationOptions> options,
        SecurityKey securityKey
    )
    {
        string? hashedPassword = await authenticationStore.GetPasswordHash(request.UserName);

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

        var token = new JwtSecurityToken(
            claims: new[] { new Claim(ClaimTypes.Name, request.UserName) },
            expires: dateTime.Now + options.CurrentValue.TokenLifetime,
            signingCredentials: new SigningCredentials(securityKey, SecurityAlgorithms.RsaSha256)
        );

        string tokenString = new JwtSecurityTokenHandler().WriteToken(token);

        var cookieOptions = new CookieOptions
        {
            SameSite = SameSiteMode.Strict,
            Secure = true,
            HttpOnly = true,
            Path = "/",
            MaxAge = options.CurrentValue.TokenLifetime,
        };

        context.Response.Cookies.Append("__Host-jwt", tokenString, cookieOptions);

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
}
