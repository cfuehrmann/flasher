using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;

using Flasher.Injectables;
using Flasher.Store.Cards;

namespace Flasher.Host.Controllers
{
    [Authorize]
    public class HistoryController : ControllerBase
    {
        private readonly ICardStore _store;
        private readonly IDateTime _time;
        private readonly IOptionsMonitor<CardsOptions> _optionsMonitor;

        public HistoryController(ICardStore store, IDateTime time, IOptionsMonitor<CardsOptions> optionsMonitor)
        {
            _store = store;
            _time = time;
            _optionsMonitor = optionsMonitor;
        }

        [HttpDelete]
        [Route("/[controller]/{id}")]
        public async Task<ActionResult<bool>> Delete(string id)
        {
            var now = _time.Now;

            var update = new CardUpdate(id)
            {
                state = State.New,
                changeTime = now,
                nextTime = now.Add(_optionsMonitor.CurrentValue.NewCardWaitingTime)
            };

            var cardWasFound = await _store.Update(User.Identity.Name!, update);

            if (cardWasFound)
                return Ok();

            return NotFound();
        }
    }
}
