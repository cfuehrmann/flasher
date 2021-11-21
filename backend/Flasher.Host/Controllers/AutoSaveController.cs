using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;

using Flasher.Store.AutoSaving;
using Flasher.Host.Model;

namespace Flasher.Host.Controllers;

[Authorize]
[Route("[controller]")]
public class AutoSaveController : ControllerBase
{
  private readonly IAutoSaveStore _store;

  public AutoSaveController(IAutoSaveStore store) => _store = store;

  [HttpDelete]
  public async Task Delete() => await _store.Delete(User.Identity!.Name!);

  [HttpPut]
  public async Task Write(WriteAutoSaveRequest request) =>
      await _store.Write(User.Identity!.Name!, new AutoSave(request.id, request.prompt, request.solution));
}