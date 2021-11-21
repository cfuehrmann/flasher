using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;

using Flasher.Injectables;
using Flasher.Store.Cards;

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
      state = State.New,
      changeTime = now,
      nextTime = now.Add(_optionsMonitor.CurrentValue.NewCardWaitingTime)
    };
    var result = await _store.Update(User.Identity!.Name!, update);
    return result != null ? Ok(result) : NotFound();
  }
}
