using System.Text.RegularExpressions;

namespace Flasher.Integration.Tests;

public static partial class VerifyExtensions
{
    [GeneratedRegex(@"__Host-jwt=([A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)")]
    private static partial Regex JwtTokenPattern();

    public static SettingsTask ScrubHostJwt(this SettingsTask settingsTask)
    {
        return settingsTask.AddScrubber(static target =>
        {
            string input = target.ToString();
            string scrubbed = JwtTokenPattern().Replace(input, "__Host-jwt=scrubbed");

            _ = target.Clear();
            _ = target.Append(scrubbed);
        });
    }
}
