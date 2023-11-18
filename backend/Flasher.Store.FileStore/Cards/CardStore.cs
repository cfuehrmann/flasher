using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using Flasher.Injectables;
using Flasher.Store.Cards;
using Flasher.Store.Exceptions;
using Microsoft.Extensions.Options;

namespace Flasher.Store.FileStore.Cards;

public class CardStore : ICardStore
{
    private readonly string _directory;
    private readonly JsonSerializerContext _jsonContext;
    private readonly ConcurrentDictionary<
        string,
        ConcurrentDictionary<string, CachedCard>
    > _cardsByUser;
    private readonly IDateTime _time;

    public CardStore(
        IOptionsMonitor<FileStoreOptions> options,
        IDateTime time,
        IFileStoreJsonContextProvider jsonContextProvider
    )
    {
        _directory =
            options.CurrentValue.Directory
            ?? throw new ArgumentException("Missing configuration 'FileStore:Directory'");
        _jsonContext = jsonContextProvider.Instance;
        const int lowestPrimeAboveInitialNumberOfUsers = 2;
        _cardsByUser = new ConcurrentDictionary<string, ConcurrentDictionary<string, CachedCard>>(
            Environment.ProcessorCount * 2,
            lowestPrimeAboveInitialNumberOfUsers
        );
        _time = time;
    }

    public Task Create(string user, FullCard card)
    {
        // Here we handle synchronous stuff early (Sonar recommendation)
        ConcurrentDictionary<string, CachedCard> cards = EnsureCache(user);
        var cachedCard = new CachedCard(
            card.Id,
            card.Prompt,
            card.Solution,
            card.State,
            card.ChangeTime,
            card.NextTime,
            card.Disabled
        );
        return cards.TryAdd(card.Id, cachedCard)
            ? WriteCards(user)
            : throw new ArgumentException($"The card with id {card.Id} already exists!");
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
            .Values
            .Where(
                card =>
                    card.Prompt?.Contains(searchText, StringComparison.OrdinalIgnoreCase) == true
                    || card.Solution?.Contains(searchText, StringComparison.OrdinalIgnoreCase)
                        == true
            )
            .OrderBy(card => card.Disabled)
            .ThenBy(card => card.NextTime)
            .ThenBy(card => card.Id)
            .Select(card => card.ToResponse());

        FullCard[] allHitsArray = allHits.ToArray();
        var result = new FindResponse
        {
            Cards = allHitsArray.Skip(skip).Take(take),
            Count = allHitsArray.Length
        };
        return Task.FromResult(result);
    }

    public Task<FullCard?> FindNext(string user)
    {
        FullCard? result = EnsureCache(user)
            .Values
            .Where(card => card.NextTime <= _time.Now && !card.Disabled)
            .OrderBy(card => card.NextTime)
            .ThenBy(card => card.Id)
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
                return new ConcurrentDictionary<string, CachedCard>(dictionary);
            }
        );
    }

    private static IEnumerable<CachedCard> GetCachedCards(
        IEnumerable<SerializableCard> deserializedCards
    )
    {
        foreach (SerializableCard card in deserializedCards)
        {
            if (
                card.Id != null
                && card.Prompt != null
                && card.Solution != null
                && card.State != null
                && card.ChangeTime != null
                && card.NextTime != null
                && card.Disabled != null
            )
            {
                yield return new CachedCard(
                    id: card.Id,
                    prompt: card.Prompt,
                    solution: card.Solution,
                    state: card.State.Value,
                    changeTime: card.ChangeTime.Value,
                    nextTime: card.NextTime.Value,
                    disabled: card.Disabled.Value
                );
            }
        }
    }

    private async Task WriteCards(string user)
    {
        string path = GetPath(user);
        try
        {
            using var fs = new FileStream(
                path,
                FileMode.Create,
                FileAccess.Write,
                FileShare.None,
                131072,
                true
            );
            ICollection<CachedCard> values = _cardsByUser[user].Values;
            var type = typeof(IEnumerable<CachedCard>);
            await JsonSerializer.SerializeAsync(fs, values, type, _jsonContext);
        }
        catch (IOException)
        {
            throw new ConflictException(
                "Cannot access the cards file. Did you use Flasher concurrently?"
            );
        }
    }

    private string GetPath(string user)
    {
        return Path.Combine(_directory, user, "cards.json");
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
            Disabled = storedCard.Disabled
        };
    }
}
