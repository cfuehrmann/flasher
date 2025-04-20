using System.Text.Json.Serialization;
using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards;

public sealed record SerializableCard
{
    public required string Id { get; set; }

    public required string Prompt { get; set; }

    public required string Solution { get; set; }

    [JsonConverter(typeof(JsonStringEnumConverter<State>))]
    public State State { get; set; }

    public DateTime ChangeTime { get; set; }

    public DateTime NextTime { get; set; }

    public bool Disabled { get; set; }
}
