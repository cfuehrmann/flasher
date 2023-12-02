using Flasher.Injectables;
using Flasher.Store.AutoSaving;
using Flasher.Store.Cards;
using Microsoft.AspNetCore.Http.HttpResults;
using Microsoft.Extensions.Options;

namespace Flasher.Host.AOT.Handlers.Cards;

public static class CardsHandler
{
    public static async Task<Created<FullCard>> Create(
        HttpContext context,
        CreateCardRequest request,
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
        return TypedResults.Created(nameof(Created), card);
    }

    public static async Task<Results<Ok<FullCard>, NotFound>> Read(
        string id,
        HttpContext context,
        ICardStore store
    )
    {
        FullCard? result = await store.Read(context.User.Identity!.Name!, id);
        return result != null ? TypedResults.Ok(result) : TypedResults.NotFound();
    }

    public static async Task<Results<Ok, NotFound>> Update(
        string id,
        UpdateCardRequest request,
        HttpContext context,
        ICardStore store,
        IAutoSaveStore autoSaveStore
    )
    {
        var update = new CardUpdate
        {
            Id = id,
            Prompt = request.Prompt,
            Solution = request.Solution
        };
        var cardWasFound = await store.Update(context.User.Identity!.Name!, update) != null;
        await autoSaveStore.Delete(context.User.Identity.Name!);
        return cardWasFound ? TypedResults.Ok() : TypedResults.NotFound();
    }

    public static async Task<Results<NoContent, NotFound>> Delete(
        string id,
        HttpContext context,
        ICardStore store
    )
    {
        return await store.Delete(context.User.Identity!.Name!, id)
            ? TypedResults.NoContent()
            : TypedResults.NotFound();
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

    public static async Task<Results<Ok<FullCard>, NotFound, NoContent>> SetOk(
        string id,
        HttpContext context,
        ICardStore store,
        IDateTime time,
        IOptionsMonitor<CardsOptions> optionsMonitor
    )
    {
        bool results = await SetState(
            id,
            State.Ok,
            optionsMonitor.CurrentValue.OkMultiplier,
            context,
            store,
            time
        );

        if (results)
        {
            FullCard? result = await store.FindNext(context.User.Identity!.Name!);
            return result != null ? TypedResults.Ok(result) : TypedResults.NoContent();
        }

        return TypedResults.NotFound();
    }

    private static async Task<bool> SetState(
        string id,
        State state,
        double multiplier,
        HttpContext context,
        ICardStore store,
        IDateTime time
    )
    {
        FullCard? card = await store.Read(context.User.Identity!.Name!, id);

        if (card == null)
        {
            return false;
        }

        DateTime now = time.Now;
        TimeSpan passedTime = now - card.ChangeTime;
        var update = new CardUpdate
        {
            Id = id,
            State = state,
            ChangeTime = now,
            NextTime = now.Add(passedTime * multiplier)
        };
        return await store.Update(context.User.Identity.Name!, update) != null;
    }
}
