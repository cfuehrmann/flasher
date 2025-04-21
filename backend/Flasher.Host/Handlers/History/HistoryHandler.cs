using Flasher.Injectables;
using Flasher.Store.Cards;
using Microsoft.AspNetCore.Http.HttpResults;
using Microsoft.Extensions.Options;

namespace Flasher.Host.Handlers.History;

public static class HistoryHandler
{
    public static async Task<Results<Ok<FullCard>, NotFound>> Delete(
        string id,
        HttpContext context,
        ICardStore store,
        IOptionsMonitor<CardsOptions> optionsMonitor,
        IDateTime time
    )
    {
        DateTime now = time.Now;

        var update = new CardUpdate
        {
            Id = id,
            State = State.New,
            ChangeTime = now,
            NextTime = now.Add(optionsMonitor.CurrentValue.NewCardWaitingTime),
        };

        FullCard? result = await store.Update(context.User.Identity!.Name!, update);

        // To maintain its cache, the frontend really must distinguish between
        // the two case below.
        return result != null ? TypedResults.Ok(result) : TypedResults.NotFound();
    }
}
