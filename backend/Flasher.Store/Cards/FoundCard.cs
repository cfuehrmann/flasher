namespace Flasher.Store.Cards
{
    public sealed record FoundCard
    {
        public FoundCard(string id, string prompt, string solution, bool disabled) =>
            (this.id, this.prompt, this.solution, this.disabled) = (id, prompt, solution, disabled);

        public string id { get; }
        public string prompt { get; }
        public string solution { get; }
        public bool disabled { get; }
    }
}