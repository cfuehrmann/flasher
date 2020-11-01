namespace Flasher.Host.Model
{
    public sealed record LoginRequest
    {
#nullable disable warnings
        public string userName { get; init; }
        public string password { get; init; }
    }
}