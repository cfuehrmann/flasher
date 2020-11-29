namespace Flasher.Store.Cards
{
    public sealed record FoundCard
    {
        public FoundCard(string id, string prompt, bool disabled) =>
            (this.id, this.prompt, this.disabled) = (id, prompt, disabled);

        public string id { get; }
        public string prompt { get; }
        public bool disabled { get; }
    }
}