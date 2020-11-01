namespace Flasher.Host.Model
{
    public sealed record UpdateCardRequest
    {
#nullable disable warnings
        public string? prompt { get; init; }
        public string? solution { get; init; }
    }
}