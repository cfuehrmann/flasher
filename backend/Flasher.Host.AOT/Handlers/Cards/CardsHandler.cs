using Flasher.Injectables;
using Flasher.Store.Cards;
using Microsoft.AspNetCore.Http.HttpResults;
using Microsoft.Extensions.Options;

namespace Flasher.Host.AOT.Handlers.Cards;

public static class CardsHandler
{
    public static async Task<Created> Create(
        CreateCardRequest request,
        HttpContext context,
        ICardStore store,
        IOptionsMonitor<CardsOptions> optionsMonitor,
        IDateTime time
    )
    {
        string id = Guid.NewGuid().ToString();
        DateTime now = time.Now;
        DateTime nextTime = now.Add(optionsMonitor.CurrentValue.NewCardWaitingTime);
        var card = new FullCard
        {
            Id = id,
            Prompt = request.Prompt,
            Solution = request.Solution,
            State = State.New,
            ChangeTime = now,
            NextTime = nextTime,
            Disabled = true
        };
        await store.Create(context.User.Identity!.Name!, card);
        return TypedResults.Created();
    }

    public static async Task<Ok<FindResponse>> Find(
        string? searchText,
        int? skip,
        HttpContext context,
        ICardStore store,
        IOptionsMonitor<CardsOptions> optionsMonitor
    )
    {
        int take = optionsMonitor.CurrentValue.PageSize;

        FindResponse result = await store.Find(
            context.User.Identity!.Name!,
            searchText ?? "",
            skip ?? 0,
            take
        );

        return TypedResults.Ok(result);
    }

    public static async Task<Results<Ok<FullCard>, NoContent>> Next(
        HttpContext context,
        ICardStore store
    )
    {
        FullCard? result = await store.FindNext(context.User.Identity!.Name!);
        return result != null ? TypedResults.Ok(result) : TypedResults.NoContent();
    }
}
