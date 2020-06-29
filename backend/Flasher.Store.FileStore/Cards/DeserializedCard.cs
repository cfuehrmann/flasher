using System;

using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards
{
    public class DeserializedCard
    {
        public string? id { get; set; }
        public string? prompt { get; set; }
        public string? solution { get; set; }
        public State? state { get; set; }
        public DateTime? changeTime { get; set; }
        public DateTime? nextTime { get; set; }
        public bool? disabled { get; set; }
    }
}