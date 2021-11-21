using System;

using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards;

public sealed record DeserializedCard
{
    public string? id { get; init; }
    public string? prompt { get; init; }
    public string? solution { get; init; }
    public State? state { get; init; }
    public DateTime? changeTime { get; init; }
    public DateTime? nextTime { get; init; }
    public bool? disabled { get; init; }
}