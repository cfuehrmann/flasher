using System.Collections.Generic;

namespace Flasher.Store.Cards
{
    public class FindResponse
    {
        public FindResponse(IEnumerable<FoundCard> cards)
        {
            this.cards = cards;
        }
        
        public IEnumerable<FoundCard> cards { get; }
    }
}