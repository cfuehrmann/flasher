using System;

namespace Flasher.Store.Cards
{
    public record CardUpdate
    {
        public CardUpdate(string id) => this.id = id;

        public string id { get; }
        public string? prompt { get; init; }
        public string? solution { get; init; }
        public State? state { get; init; }
        public DateTime? changeTime { get; init; }
        public DateTime? nextTime { get; init; }
        public bool? disabled { get; init; }
    }
}