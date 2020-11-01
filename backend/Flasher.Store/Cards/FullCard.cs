using System;

namespace Flasher.Store.Cards
{
    public sealed record FullCard
    {
        public FullCard(
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
        public string prompt { get; }
        public string solution { get; }
        public State state { get; }
        public DateTime changeTime { get; }
        public DateTime nextTime { get; }
        public bool disabled { get; }
    }
}