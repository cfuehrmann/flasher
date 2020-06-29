namespace Flasher.Store.Cards
{
    public class FoundCard
    {

        public FoundCard(string id, string prompt, string solution, bool disabled)
        {
            this.id = id;
            this.prompt = prompt;
            this.solution = solution;
            this.disabled = disabled;
        }

        public string id { get; }
        public string prompt { get; }
        public string solution { get; }
        public bool disabled { get; }
    }
}