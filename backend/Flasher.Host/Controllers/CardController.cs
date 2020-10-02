using System;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;

using Flasher.Host.Model;
using Flasher.Injectables;
using Flasher.Store.AutoSaving;
using Flasher.Store.Cards;

namespace Flasher.Host.Controllers
{
    [Authorize]
    public class CardsController : ControllerBase
    {
        private readonly ICardStore _store;
        private readonly IAutoSaveStore _autoSaveStore;
        private readonly IDateTime _time;
        private readonly IOptionsMonitor<CardsOptions> _optionsMonitor;

        public CardsController(ICardStore store, IAutoSaveStore autoSaveStore, IDateTime time, IOptionsMonitor<CardsOptions> optionsMonitor)
        {
            _store = store;
            _autoSaveStore = autoSaveStore;
            _time = time;
            _optionsMonitor = optionsMonitor;
        }

        [HttpPost]
        [Route("/[controller]")]
        public async Task<ActionResult> Create(CreateCardRequest request)
        {
            var id = Guid.NewGuid().ToString();
            var now = _time.Now;
            var nextTime = now.Add(_optionsMonitor.CurrentValue.NewCardWaitingTime);
            var card = new FullCard(id, request.prompt, request.solution, State.New, now, nextTime, true);

            await _store.Create(User.Identity.Name!, card);

            return CreatedAtAction(nameof(Read), new { id = id }, card);
        }

        [HttpGet]
        [Route("/[controller]/{id}")]
        public async Task<ActionResult<FullCard>> Read(string id)
        {
            var result = await _store.Read(User.Identity.Name!, id);

            if (result != null) return result;

            return NotFound();
        }

        [HttpPatch]
        [Route("/[controller]/{id}")]
        public async Task<ActionResult<bool>> Update(string id, UpdateCardRequest request)
        {
            var now = _time.Now;
            var update = new CardUpdate(id) { prompt = request.prompt, solution = request.solution };
            var cardWasFound = await _store.Update(User.Identity.Name!, update);

            await _autoSaveStore.Delete(User.Identity.Name!);

            if (cardWasFound)
                return Ok();

            return NotFound();
        }

        [HttpDelete]
        [Route("/[controller]/{id}")]
        public async Task<ActionResult> Delete(string id)
        {
            var cardWasFound = await _store.Delete(User.Identity.Name!, id);

            if (cardWasFound)
                return NoContent();

            return NotFound();
        }

        [HttpGet]
        [Route("/[controller]")]
        public async Task<FindResponse> Find(string? searchText)
        {
            return await _store.Find(User.Identity.Name!, searchText ?? "");
        }

        [HttpGet]
        [Route("/[controller]/[action]")]
        public async Task<ActionResult<FullCard>> Next()
        {
            var result = await _store.FindNext(User.Identity.Name!);

            if (result != null) return result;

            return NoContent();
        }

        [HttpPost]
        [Route("/[controller]/{id}/[action]")]
        public async Task<ActionResult> SetOk(string id) =>
            await SetState(id, State.Ok, _optionsMonitor.CurrentValue.OkMultiplier);

        [HttpPost]
        [Route("/[controller]/{id}/[action]")]
        public async Task<ActionResult> SetFailed(string id) =>
            await SetState(id, State.Failed, _optionsMonitor.CurrentValue.FailedMultiplier);

        [HttpPost]
        [Route("/[controller]/{id}/[action]")]
        public async Task<ActionResult> Enable(string id)
        {
            var update = new CardUpdate(id) { disabled = false };

            var cardWasFound = await _store.Update(User.Identity.Name!, update);

            if (cardWasFound)
                return NoContent();

            return NotFound();
        }

        [HttpPost]
        [Route("/[controller]/{id}/[action]")]
        public async Task<ActionResult> Disable(string id)
        {
            var update = new CardUpdate(id) { disabled = true };

            var cardWasFound = await _store.Update(User.Identity.Name!, update);

            if (cardWasFound)
                return NoContent();

            return NotFound();
        }

        private async Task<ActionResult> SetState(string id, State state, double multiplier)
        {
            var card = await _store.Read(User.Identity.Name!, id);

            if (card == null) return NotFound();

            var now = _time.Now;
            var passedTime = now - card.changeTime;

            var update = new CardUpdate(id)
            {
                state = state,
                changeTime = now,
                nextTime = now.Add(passedTime * multiplier)
            };

            var cardWasFound = await _store.Update(User.Identity.Name!, update);

            if (cardWasFound)
                return NoContent();

            return NotFound();
        }
    }
}
