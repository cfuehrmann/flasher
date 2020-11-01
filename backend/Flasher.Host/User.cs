namespace Flasher.Host
{
    public sealed record User
    {
        public User(string name) => this.Name = name;

        public string Name { get; }
    }
}