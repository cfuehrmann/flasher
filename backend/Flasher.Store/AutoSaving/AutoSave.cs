namespace Flasher.Store.AutoSaving
{
    public sealed record AutoSave
    {
        public AutoSave(string id, string prompt, string solution) => 
            (this.id, this.prompt, this.solution) = (id, prompt, solution);

        public string id { get; }
        public string prompt { get; }
        public string solution { get; }
    }
}