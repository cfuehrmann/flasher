using System;

using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards
{
    public sealed record CachedCard
    {
        public CachedCard(
            string id, string prompt, string solution, State state, DateTime changeTime, DateTime nextTime, bool disabled)
        {
            this.id = id;
            this.prompt = prompt;
            this.solution = solution;
            this.state = state;
            this.changeTime = changeTime;
            this.nextTime = nextTime;
            this.disabled = disabled;
        }

        public string id { get; }
        public string prompt { get; set; } // intentionally mutable!
        public string solution { get; set; }
        public State state { get; set; }
        public DateTime changeTime { get; set; }
        public DateTime nextTime { get; set; }
        public bool disabled { get; set; }
    }
}