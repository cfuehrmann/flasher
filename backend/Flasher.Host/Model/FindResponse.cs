using System.Collections.Generic;
using Flasher.Store.Cards;

namespace Flasher.Host.Model
{
    public sealed record FindResponse
    {
        public FindResponse(IEnumerable<FoundCard> cards, int count, int pageCount) =>
            (this.cards, this.count, this.pageCount) = (cards, count, pageCount);

        public IEnumerable<FoundCard> cards { get; }

        public int count { get; }

        public int pageCount { get; }
    }
}