using System.Collections.Generic;

namespace Flasher.Store.Cards
{
    public sealed record FindResponse
    {
        public FindResponse(IEnumerable<FoundCard> cards) => this.cards = cards;
        
        public IEnumerable<FoundCard> cards { get; }
    }
}