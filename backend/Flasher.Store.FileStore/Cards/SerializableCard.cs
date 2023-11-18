using System;
using System.Text.Json.Serialization;
using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards;

public sealed record SerializableCard
{
    public string? Id { get; set; }
    public string? Prompt { get; set; }
    public string? Solution { get; set; }

    [JsonConverter(typeof(JsonStringEnumConverter<State>))]
    public State? State { get; set; }

    public DateTime? ChangeTime { get; set; }
    public DateTime? NextTime { get; set; }
    public bool? Disabled { get; set; }
}
