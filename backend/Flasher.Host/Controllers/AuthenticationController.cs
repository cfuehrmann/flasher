using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Identity;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

using Flasher.Host.Model;
using Flasher.Injectables;
using Flasher.Store.Authentication;
using Flasher.Store.AutoSaving;

namespace Flasher.Host.Controllers;

[Route("[controller]/[action]")]
public class AuthenticationController : ControllerBase
{
    private readonly IAuthenticationStore _store;
    private readonly IAutoSaveStore _autoSaveStore;
    private readonly RsaSecurityKey _securityKey;
    private readonly IPasswordHasher<User> _passwordHasher;
    private readonly AuthenticationOptions _options;
    private readonly IDateTime _dateTime;

    public AuthenticationController(IAuthenticationStore store, IAutoSaveStore autoSaveStore,
        RsaSecurityKey securityKey, IPasswordHasher<User> passwordHasher,
        IOptionsMonitor<AuthenticationOptions> options, IDateTime dateTime)
    {
        _store = store;
        _autoSaveStore = autoSaveStore;
        _securityKey = securityKey;
        _passwordHasher = passwordHasher;
        _options = options.CurrentValue;
        _dateTime = dateTime;
    }

    [AllowAnonymous]
    [HttpPost]
    public async Task<ActionResult<LoginResponse>> Login(LoginRequest request)
    {
        var hashedPassword = await _store.GetPasswordHash(request.userName);
        if (hashedPassword == null) return Challenge();
        var passwordVerificationResult =
            _passwordHasher.VerifyHashedPassword(new User(request.userName), hashedPassword, request.password);
        if (passwordVerificationResult != PasswordVerificationResult.Success) return Challenge();
        var readAutoSave = _autoSaveStore.Read(request.userName);
        var tokenString = GetTokenString(request.userName);
        var options = new CookieOptions
        {
            SameSite = SameSiteMode.Strict,
            Secure = true,
            HttpOnly = true,
            Path = "/",
            MaxAge = _options.TokenLifetime,
        };
        HttpContext.Response.Cookies.Append("__Host-jwt", tokenString, options);
        var autoSave = await readAutoSave;
        return new LoginResponse(tokenString) { autoSave = autoSave };
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