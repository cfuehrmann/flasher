using System;

using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards;

public sealed record DeserializedCard
{
    public string? Id { get; init; }
    public string? Prompt { get; init; }
    public string? Solution { get; init; }
    public State? State { get; init; }
    public DateTime? ChangeTime { get; init; }
    public DateTime? NextTime { get; init; }
    public bool? Disabled { get; init; }
}
