using System.Collections.Generic;

namespace Flasher.Store.Cards;

public sealed record FindResponse(IEnumerable<FullCard> Cards, int Count);
