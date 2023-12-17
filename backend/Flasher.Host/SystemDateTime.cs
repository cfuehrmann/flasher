using Flasher.Injectables;

namespace Flasher.Host;

public sealed class SystemDateTime : IDateTime
{
    public DateTime Now => DateTime.Now;
}
