using System.Collections.Generic;

namespace Flasher.Store.Cards
{
    public sealed record FindResponse
    {
        public FindResponse(IEnumerable<FoundCard> cards, int count) =>
            (this.cards, this.count) = (cards, count);

        public IEnumerable<FoundCard> cards { get; }

        public int count { get; }
    }
}