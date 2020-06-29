using System;

namespace Flasher.Store.Cards
{
    public class CardUpdate
    {
        public CardUpdate(string id)
        {
            this.id = id;
        }
        
        public string id { get;  }
        public string? prompt { get; set; }
        public string? solution { get; set; }
        public State? state { get; set; }
        public DateTime? changeTime { get; set; }
        public DateTime? nextTime { get; set; }
        public bool? disabled { get; set; }
    }
}