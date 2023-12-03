namespace Flasher.Host.Handlers.Authentication;

public sealed record LoginResponse
{
    public required string JsonWebToken { get; init; }
    public AutoSaveData? AutoSave { get; init; }

    public sealed record AutoSaveData
    {
        public required string Id { get; init; }
        public required string Prompt { get; init; }
        public required string Solution { get; init; }
    }
}
