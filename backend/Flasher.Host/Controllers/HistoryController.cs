using System.Threading.Tasks;

using Flasher.Injectables;
using Flasher.Store.Cards;

using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;

namespace Flasher.Host.Controllers;

[Authorize]
public class HistoryController : ControllerBase
{
    private readonly ICardStore _store;
    private readonly IDateTime _time;
    private readonly IOptionsMonitor<CardsOptions> _optionsMonitor;

    public HistoryController(ICardStore store, IDateTime time, IOptionsMonitor<CardsOptions> optionsMonitor) =>
        (_store, _time, _optionsMonitor) = (store, time, optionsMonitor);

    [HttpDelete]
    [Route("/[controller]/{id}")]
    public async Task<ActionResult<FullCard>> Delete(string id)
    {
        var now = _time.Now;
        var update = new CardUpdate(id)
        {
            State = State.New,
            ChangeTime = now,
            NextTime = now.Add(_optionsMonitor.CurrentValue.NewCardWaitingTime)
        };
        var result = await _store.Update(User.Identity!.Name!, update);
        return result != null ? Ok(result) : NotFound();
    }
}
