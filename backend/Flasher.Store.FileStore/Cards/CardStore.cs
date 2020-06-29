using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using Microsoft.Extensions.Options;

using Flasher.Injectables;
using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards
{
    public class CardStore : ICardStore
    {
        private readonly IList<CachedCard> _cards;
        private readonly IDateTime _time;
        private readonly string _path;
        private readonly JsonSerializerOptions _jsonOptions;

        public CardStore(IOptionsMonitor<FileStoreOptions> options, IDateTime time)
        {
            if (options.CurrentValue.Directory == null)
                throw new Exception("Missing configuration 'FileStore:Directory'");

            _path = Path.Combine(options.CurrentValue.Directory, "cards.json");
            var json = File.ReadAllText(_path);

            _jsonOptions = new JsonSerializerOptions { WriteIndented = true };
            _jsonOptions.Converters.Add(new JsonStringEnumConverter());

            var deserializedCards = JsonSerializer.Deserialize<IList<DeserializedCard>>(json, _jsonOptions);

            IEnumerable<CachedCard> cards()
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

            _cards = cards().ToList();

            _time = time;
        }

        public async Task Create(string user, FullCard card)
        {
            if (_cards.Any(c => c.id == card.id))
                throw new ArgumentException($"The card with id {card.id} already exists!");

            var cachedCard = new CachedCard(card.id, card.prompt, card.solution, card.state, card.changeTime, card.nextTime, card.disabled);

            _cards.Add(cachedCard);

            await WriteCards();
        }

        public Task<FullCard?> Read(string user, string id)
        {
            var result = _cards.FirstOrDefault(card => card.id == id).ToResponse();

            return Task.FromResult(result);
        }

        public async Task<bool> Update(string user, CardUpdate update)
        {
            var card = _cards.FirstOrDefault(c => c.id == update.id);

            if (card == null) return false;

            if (update.prompt != null) card.prompt = update.prompt;
            if (update.solution != null) card.solution = update.solution;
            if (update.state is State state) card.state = state;
            if (update.changeTime is DateTime changeTime) card.changeTime = changeTime;
            if (update.nextTime is DateTime nextTime) card.nextTime = nextTime;
            if (update.disabled is bool disabled) card.disabled = disabled;

            await WriteCards();

            return true;
        }

        public async Task<bool> Delete(string user, string id)
        {
            var hits = _cards.Select((card, index) => (card, index)).Where(pair => pair.card.id == id);

            if (!hits.Any()) return false;

            _cards.RemoveAt(hits.Single().index);
            await WriteCards();

            return true;
        }

        public Task<FindResponse> Find(string user, string searchText)
        {
            var result =
                from card in _cards
                where card.prompt != null && card.solution != null &&
                    (card.prompt.Contains(searchText, StringComparison.OrdinalIgnoreCase) ||
                        card.solution.Contains(searchText, StringComparison.OrdinalIgnoreCase))
                select new FoundCard(card.id, card.prompt, card.solution, card.disabled);

            return Task.FromResult(new FindResponse(result));
        }

        public Task<FullCard?> FindNext(string user)
        {
            var result = _cards
                .Where(card => card.nextTime <= _time.Now && !card.disabled)
                .OrderBy(card => card.nextTime)
                .FirstOrDefault()
                .ToResponse();

            return Task.FromResult(result);
        }

        private async Task WriteCards()
        {
            using var fs = File.Create(_path, 131072, FileOptions.Asynchronous);
            await JsonSerializer.SerializeAsync<IList<CachedCard>>(fs, _cards, _jsonOptions);
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