namespace Flasher.Store.AutoSaving
{
    public class AutoSave
    {
        public AutoSave(string id, string prompt, string solution)
        {
            this.id = id;
            this.prompt = prompt;
            this.solution = solution;
        }

        public string id { get; }
        public string prompt { get; }
        public string solution { get; }
    }
}