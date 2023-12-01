using Flasher.Injectables;

namespace Flasher.Host.AOT;

public sealed class SystemDateTime : IDateTime
{
    public DateTime Now => DateTime.Now;
}
