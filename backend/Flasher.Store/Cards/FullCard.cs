using System;

namespace Flasher.Store.Cards
{
    public record FullCard(
        string id,
        string prompt,
        string solution,
        State state,
        DateTime changeTime,
        DateTime nextTime,
        bool disabled);
}