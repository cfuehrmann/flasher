using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using Microsoft.Extensions.Options;

using Flasher.Injectables;
using Flasher.Store.Cards;
using Flasher.Store.Exceptions;

namespace Flasher.Store.FileStore.Cards
{
    public class CardStore : ICardStore
    {
        private readonly string _directory;
        private readonly JsonSerializerOptions _jsonOptions;
        private readonly ConcurrentDictionary<string, ConcurrentDictionary<string, CachedCard>> _cardsByUser;
        private readonly IDateTime _time;

        public CardStore(IOptionsMonitor<FileStoreOptions> options, IDateTime time)
        {
            if (options.CurrentValue.Directory == null)
                throw new Exception("Missing configuration 'FileStore:Directory'");

            _directory = options.CurrentValue.Directory;

            _jsonOptions = new JsonSerializerOptions { WriteIndented = true };
            _jsonOptions.Converters.Add(new JsonStringEnumConverter());

            const int lowestPrimeAboveInitialNumberOfUsers = 2;

            _cardsByUser = new ConcurrentDictionary<string, ConcurrentDictionary<string, CachedCard>>(
                Environment.ProcessorCount * 2, lowestPrimeAboveInitialNumberOfUsers);

            _time = time;
        }

        public async Task Create(string user, FullCard card)
        {
            var cards = EnsureCache(user);

            var cachedCard = new CachedCard(card.id, card.prompt, card.solution, card.state,
                card.changeTime, card.nextTime, card.disabled);

            if (!cards.TryAdd(card.id, cachedCard))
                throw new ArgumentException($"The card with id {card.id} already exists!");

            await WriteCards(user);
        }

        public Task<FullCard?> Read(string user, string id)
        {
            var cards = EnsureCache(user);

            var result = cards.TryGetValue(id, out var cachedCard) ?
                cachedCard.ToResponse() :
                null;

            return Task.FromResult(result);
        }

        public async Task<bool> Update(string user, CardUpdate update)
        {
            var cards = EnsureCache(user);

            if (!cards.TryGetValue(update.id, out var card))
                return false;

            if (update.prompt != null) card.prompt = update.prompt;
            if (update.solution != null) card.solution = update.solution;
            if (update.state is State state) card.state = state;
            if (update.changeTime is DateTime changeTime) card.changeTime = changeTime;
            if (update.nextTime is DateTime nextTime) card.nextTime = nextTime;
            if (update.disabled is bool disabled) card.disabled = disabled;

            await WriteCards(user);

            return true;
        }

        public async Task<bool> Delete(string user, string id)
        {
            var result = EnsureCache(user).TryRemove(id, out _);
            await WriteCards(user);
            return result;
        }

        public Task<FindResponse> Find(string user, string searchText)
        {
            var cards = EnsureCache(user);

            var result =
                from card in cards.Values
                where card.prompt != null && card.solution != null &&
                    (card.prompt.Contains(searchText, StringComparison.OrdinalIgnoreCase) ||
                        card.solution.Contains(searchText, StringComparison.OrdinalIgnoreCase))
                select new FoundCard(card.id, card.prompt, card.solution, card.disabled);

            return Task.FromResult(new FindResponse(result));
        }

        public Task<FullCard?> FindNext(string user)
        {
            var cards = EnsureCache(user);

            var result = cards.Values
                .Where(card => card.nextTime <= _time.Now && !card.disabled)
                .OrderBy(card => card.nextTime)
                .FirstOrDefault()
                .ToResponse();

            return Task.FromResult(result);
        }

        private ConcurrentDictionary<string, CachedCard> EnsureCache(string user)
        {
            return _cardsByUser.GetOrAdd(user, user =>
            {
                var path = GetPath(user);
                var json = File.ReadAllText(path);
                var deserializedCards = JsonSerializer.Deserialize<IEnumerable<DeserializedCard>>(json, _jsonOptions);
                var dictionary = GetCachedCards(deserializedCards).ToDictionary(card => card.id);
                return new ConcurrentDictionary<string, CachedCard>(dictionary);
            });
        }

        private static IEnumerable<CachedCard> GetCachedCards(IEnumerable<DeserializedCard> deserializedCards)
        {
            foreach (var card in deserializedCards)
                if (card.id != null && card.prompt != null && card.solution != null && card.state != null &&
                    card.changeTime != null && card.nextTime != null && card.disabled != null)
                    yield return new CachedCard(
                        card.id,
                        card.prompt,
                        card.solution,
                        card.state.Value,
                        card.changeTime.Value,
                        card.nextTime.Value,
                        card.disabled.Value);
        }

        private async Task WriteCards(string user)
        {
            var path = GetPath(user);

            try
            {
                using var fs = new FileStream(path, FileMode.Create, FileAccess.Write,
                    FileShare.None, 131072, true);

                await JsonSerializer.SerializeAsync<ICollection<CachedCard>>(
                    fs, _cardsByUser[user].Values, _jsonOptions);
            }
            catch (IOException)
            {
                throw new ConflictException("Cannot access the cards file. Did you use Flasher concurrently?");
            }
        }

        private string GetPath(string user)
        {
            return Path.Combine(_directory, user, "cards.json");
        }
    }

    public static class Extensions
    {
        public static FullCard? ToResponse(this CachedCard? storedCard)
        {
            return storedCard == null ?
                null :
                new FullCard(
                    storedCard.id,
                    storedCard.prompt,
                    storedCard.solution,
                    storedCard.state,
                    storedCard.changeTime,
                    storedCard.nextTime,
                    storedCard.disabled);
        }
    }
}