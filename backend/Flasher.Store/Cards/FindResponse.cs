namespace Flasher.Store.Cards;

public sealed record FindResponse
{
    public required IEnumerable<FullCard> Cards { get; init; }
    public required int Count { get; init; }
}
