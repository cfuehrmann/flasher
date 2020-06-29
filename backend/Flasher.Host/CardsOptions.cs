using System;

namespace Flasher.Host
{
    public class CardsOptions
    {
        public TimeSpan NewCardWaitingTime { get; set; }
        public double OkMultiplier { get; set; }
        public double FailedMultiplier { get; set; }
    }
}