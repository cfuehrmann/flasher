namespace Flasher.Host.Model;
public sealed record WriteAutoSaveRequest
{
#nullable disable warnings
    public string id { get; init; }
    public string prompt { get; init; }
    public string solution { get; init; }
}