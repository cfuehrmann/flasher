using System.Collections.Concurrent;
using System.Text.Json;
using System.Text.Json.Serialization;
using Flasher.Injectables;
using Flasher.Store.Cards;
using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.Cards;

public class CardStore(
    IOptionsMonitor<FileStoreOptions> options,
    IDateTime time,
    IFileStoreJsonContextProvider jsonContextProvider
) : ICardStore
{
    private readonly JsonSerializerContext _jsonContext = jsonContextProvider.Instance;

    private readonly ConcurrentDictionary<
        string,
        ConcurrentDictionary<string, CachedCard>
    > _cardsByUser = new();

    public async Task Create(string user, FullCard card)
    {
        var cards = EnsureCache(user);

        var cachedCard = new CachedCard(
            card.Id,
            card.Prompt,
            card.Solution,
            card.State,
            card.ChangeTime,
            card.NextTime,
            card.Disabled
        );

        //  The card cannot exists already, because the handler always creates
        //  a new ID.
        _ = cards.TryAdd(card.Id, cachedCard);

        await WriteCards(user);
    }

    public Task<FullCard?> Read(string user, string id)
    {
        FullCard? result = EnsureCache(user).TryGetValue(id, out CachedCard? cachedCard)
            ? cachedCard.ToResponse()
            : null;
        return Task.FromResult(result);
    }

    public Task<FullCard?> Update(string user, CardUpdate update)
    {
        ConcurrentDictionary<string, CachedCard> cards = EnsureCache(user);

        if (!cards.TryGetValue(update.Id, out CachedCard? card))
        {
            return Task.FromResult<FullCard?>(null);
        }

        if (update.Prompt != null)
        {
            card.Prompt = update.Prompt;
        }

        if (update.Solution != null)
        {
            card.Solution = update.Solution;
        }

        if (update.State is State state)
        {
            card.State = state;
        }

        if (update.ChangeTime is DateTime changeTime)
        {
            card.ChangeTime = changeTime;
        }

        if (update.NextTime is DateTime nextTime)
        {
            card.NextTime = nextTime;
        }

        if (update.Disabled is bool disabled)
        {
            card.Disabled = disabled;
        }

        return Update(user, card);
    }

    private async Task<FullCard?> Update(string user, CachedCard card)
    {
        await WriteCards(user);
        return card.ToResponse();
    }

    public async Task<bool> Delete(string user, string id)
    {
        bool result = EnsureCache(user).TryRemove(id, out _);
        await WriteCards(user);
        return result;
    }

    public Task<FindResponse> Find(string user, string searchText, int skip, int take)
    {
        IEnumerable<FullCard> allHits = EnsureCache(user)
            .Values.Where(card =>
                card.Prompt?.Contains(searchText, StringComparison.OrdinalIgnoreCase) == true
                || card.Solution?.Contains(searchText, StringComparison.OrdinalIgnoreCase) == true
            )
            .OrderBy(card => card.Disabled)
            .ThenBy(card => card.NextTime)
            .Select(card => card.ToResponse());

        FullCard[] allHitsArray = [.. allHits];
        var result = new FindResponse
        {
            Cards = allHitsArray.Skip(skip).Take(take),
            Count = allHitsArray.Length,
        };
        return Task.FromResult(result);
    }

    public Task<FullCard?> FindNext(string user)
    {
        FullCard? result = EnsureCache(user)
            .Values.Where(card => card.NextTime <= time.Now && !card.Disabled)
            .OrderBy(card => card.NextTime)
            .FirstOrDefault()
            ?.ToResponse();

        return Task.FromResult(result);
    }

    private ConcurrentDictionary<string, CachedCard> EnsureCache(string user)
    {
        return _cardsByUser.GetOrAdd(
            user,
            user =>
            {
                string path = GetPath(user);
                string json = File.ReadAllText(path);
                var type = typeof(IEnumerable<SerializableCard>);
                object? deserialized = JsonSerializer.Deserialize(json, type, _jsonContext);
                if (deserialized is not IEnumerable<SerializableCard> typedValue)
                {
                    throw new InvalidOperationException(
                        "Deserializing the cards file returned null!"
                    );
                }

                var dictionary = GetCachedCards(typedValue).ToDictionary(card => card.Id);
                return new(dictionary);
            }
        );
    }

    private static IEnumerable<CachedCard> GetCachedCards(
        IEnumerable<SerializableCard> deserializedCards
    )
    {
        foreach (SerializableCard card in deserializedCards)
        {
            yield return new CachedCard(
                id: card.Id,
                prompt: card.Prompt,
                solution: card.Solution,
                state: card.State,
                changeTime: card.ChangeTime,
                nextTime: card.NextTime,
                disabled: card.Disabled
            );
        }
    }

    private async Task WriteCards(string user)
    {
        string path = GetPath(user);
        using var fs = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.None);
        var values = _cardsByUser[user].Values;
        var type = typeof(IEnumerable<CachedCard>);
        await JsonSerializer.SerializeAsync(fs, values, type, _jsonContext);
    }

    private string GetPath(string user)
    {
        return Path.Combine(options.CurrentValue.Directory, user, "cards.json");
    }
}

public static class Extensions
{
    public static FullCard ToResponse(this CachedCard storedCard)
    {
        return new FullCard
        {
            Id = storedCard.Id,
            Prompt = storedCard.Prompt,
            Solution = storedCard.Solution,
            State = storedCard.State,
            ChangeTime = storedCard.ChangeTime,
            NextTime = storedCard.NextTime,
            Disabled = storedCard.Disabled,
        };
    }
}
