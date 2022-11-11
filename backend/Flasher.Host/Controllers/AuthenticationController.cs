using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Threading.Tasks;

using Flasher.Host.Model;
using Flasher.Injectables;
using Flasher.Store.Authentication;
using Flasher.Store.AutoSaving;

using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Identity;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

namespace Flasher.Host.Controllers;

[Route("[controller]/[action]")]
public class AuthenticationController : ControllerBase
{
    private readonly IAuthenticationStore _store;
    private readonly IAutoSaveStore _autoSaveStore;
    private readonly SecurityKey _securityKey;
    private readonly IPasswordHasher<User> _passwordHasher;
    private readonly AuthenticationOptions _options;
    private readonly IDateTime _dateTime;

    public AuthenticationController(IAuthenticationStore store, IAutoSaveStore autoSaveStore,
        SecurityKey securityKey, IPasswordHasher<User> passwordHasher,
        IOptionsMonitor<AuthenticationOptions> options, IDateTime dateTime)
    {
        _store = store;
        _autoSaveStore = autoSaveStore;
        _securityKey = securityKey;
        _passwordHasher = passwordHasher;
        _options = options.CurrentValue;
        _dateTime = dateTime;
    }

    [HttpPost]
    public async Task<ActionResult<LoginResponse>> Login(LoginRequest request)
    {
        string? hashedPassword = await _store.GetPasswordHash(request.UserName);

        if (hashedPassword == null)
        {
            return Challenge();
        }

        PasswordVerificationResult passwordVerificationResult =
            _passwordHasher.VerifyHashedPassword(new User(request.UserName), hashedPassword, request.Password);

        if (passwordVerificationResult != PasswordVerificationResult.Success)
        {
            return Challenge();
        }

        Task<AutoSave?> readAutoSave = _autoSaveStore.Read(request.UserName);
        string tokenString = GetTokenString(request.UserName);
        var options = new CookieOptions
        {
            SameSite = SameSiteMode.Strict,
            Secure = true,
            HttpOnly = true,
            Path = "/",
            MaxAge = _options.TokenLifetime,
        };
        HttpContext.Response.Cookies.Append("__Host-jwt", tokenString, options);
        AutoSave? autoSave = await readAutoSave;
        return new LoginResponse(tokenString) { AutoSave = autoSave };
    }

    private string GetTokenString(string userName)
    {
        var token = new JwtSecurityToken(
            claims: new[] { new Claim(ClaimTypes.Name, userName) },
            expires: _dateTime.Now + _options.TokenLifetime,
            signingCredentials: new SigningCredentials(_securityKey, SecurityAlgorithms.RsaSha256)
        );
        return new JwtSecurityTokenHandler().WriteToken(token);
    }
}
