using System;

using Flasher.Injectables;

namespace Flasher.Host
{
    public class SystemDateTime : IDateTime
    {
        public DateTime Now => DateTime.Now;
    }
}