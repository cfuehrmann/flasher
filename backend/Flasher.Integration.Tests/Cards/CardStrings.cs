using System.Globalization;
using Flasher.Store.Cards;

namespace Flasher.Integration.Tests.Cards;

public record CardStrings(
    string Id,
    string Prompt,
    string Solution,
    string State,
    string ChangeTime,
    string NextTime,
    string Disabled
)
{
    public string Json =>
        $$"""
            {
                "Id": "{{Id}}",
                "Prompt": "{{Prompt}}",
                "Solution": "{{Solution}}",
                "State": "{{State}}",
                "ChangeTime": "{{ChangeTime}}",
                "NextTime": "{{NextTime}}",
                "Disabled": {{Disabled}}
            }
            """;

    public FullCard FullCard =>
        new()
        {
            Id = Id,
            Prompt = Prompt,
            Solution = Solution,
            State = Enum.Parse<State>(State),
            ChangeTime = GetDateTime(ChangeTime),
            NextTime = GetDateTime(NextTime),
            Disabled = bool.Parse(Disabled),
        };

    private static DateTime GetDateTime(string dateTimeString)
    {
        return DateTime.Parse(dateTimeString, CultureInfo.InvariantCulture);
    }
}
