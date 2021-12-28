﻿using System;
using System.Threading.Tasks;

using Flasher.Host.Model;
using Flasher.Injectables;
using Flasher.Store.AutoSaving;
using Flasher.Store.Cards;

using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;

namespace Flasher.Host.Controllers;

[Authorize]
public class CardsController : ControllerBase
{
    private readonly ICardStore _store;
    private readonly IAutoSaveStore _autoSaveStore;
    private readonly IDateTime _time;
    private readonly IOptionsMonitor<CardsOptions> _optionsMonitor;

    public CardsController(ICardStore store, IAutoSaveStore autoSaveStore, IDateTime time,
        IOptionsMonitor<CardsOptions> optionsMonitor) =>
        (_store, _autoSaveStore, _time, _optionsMonitor) = (store, autoSaveStore, time, optionsMonitor);

    [HttpPost]
    [Route("/[controller]")]
    public async Task<ActionResult> Create(CreateCardRequest request)
    {
        var id = Guid.NewGuid().ToString();
        var now = _time.Now;
        var nextTime = now.Add(_optionsMonitor.CurrentValue.NewCardWaitingTime);
        var card = new FullCard(
            Id: id,
            Prompt: request.Prompt,
            Solution: request.Solution,
            State: State.New,
            ChangeTime: now,
            NextTime: nextTime,
            Disabled: true);
        await _store.Create(User.Identity!.Name!, card);
        return CreatedAtAction(nameof(Read), new { id }, card);
    }

    [HttpGet]
    [Route("/[controller]/{id}")]
    public async Task<ActionResult<FullCard>> Read(string id)
    {
        var result = await _store.Read(User.Identity!.Name!, id);
        return result != null ? result : NotFound();
    }

    [HttpPatch]
    [Route("/[controller]/{id}")]
    public async Task<ActionResult<bool>> Update(string id, UpdateCardRequest request)
    {
        _ = _time.Now;
        var update = new CardUpdate(id) { Prompt = request.Prompt, Solution = request.Solution };
        var cardWasFound = await _store.Update(User.Identity!.Name!, update) != null;
        await _autoSaveStore.Delete(User.Identity.Name!);
        return cardWasFound ? Ok() : NotFound();
    }

    [HttpDelete]
    [Route("/[controller]/{id}")]
    public async Task<ActionResult> Delete(string id) =>
        await _store.Delete(User.Identity!.Name!, id) ? NoContent() : NotFound();

    [HttpGet]
    [Route("/[controller]")]
    public async Task<FindResponse> Find(string? searchText, int skip)
    {
        var pageSize = _optionsMonitor.CurrentValue.PageSize;
        var take = pageSize < 1 ? 15 : pageSize;
        return await _store.Find(User.Identity!.Name!, searchText ?? "", skip, take);
    }

    [HttpGet]
    [Route("/[controller]/[action]")]
    public async Task<ActionResult<FullCard>> Next()
    {
        var result = await _store.FindNext(User.Identity!.Name!);
        return result != null ? result : NoContent();
    }

    [HttpPost]
    [Route("/[controller]/{id}/[action]")]
    public async Task<ActionResult<FullCard>> SetOk(string id)
    {
        var setStateResult = await SetState(id, State.Ok, _optionsMonitor.CurrentValue.OkMultiplier);

        if (setStateResult is NoContentResult)
        {
            var result = await _store.FindNext(User.Identity!.Name!);
            return result != null ? result : NoContent();
        }

        return setStateResult;
    }

    [HttpPost]
    [Route("/[controller]/{id}/[action]")]
    public async Task<ActionResult<FullCard>> SetFailed(string id)
    {
        var setStateResult = await SetState(id, State.Failed, _optionsMonitor.CurrentValue.OkMultiplier);

        if (setStateResult is NoContentResult)
        {
            var result = await _store.FindNext(User.Identity!.Name!);
            return result != null ? result : NoContent();
        }

        return setStateResult;
    }

    [HttpPost]
    [Route("/[controller]/{id}/[action]")]
    public async Task<ActionResult> Enable(string id)
    {
        var update = new CardUpdate(id) { Disabled = false };
        return await _store.Update(User.Identity!.Name!, update) != null ? NoContent() : NotFound();
    }

    [HttpPost]
    [Route("/[controller]/{id}/[action]")]
    public async Task<ActionResult> Disable(string id)
    {
        var update = new CardUpdate(id) { Disabled = true };
        return await _store.Update(User.Identity!.Name!, update) != null ? NoContent() : NotFound();
    }

    private async Task<ActionResult> SetState(string id, State state, double multiplier)
    {
        var card = await _store.Read(User.Identity!.Name!, id);
        if (card == null) return NotFound();
        var now = _time.Now;
        var passedTime = now - card.ChangeTime;
        var update = new CardUpdate(id)
        {
            State = state,
            ChangeTime = now,
            NextTime = now.Add(passedTime * multiplier)
        };
        return await _store.Update(User.Identity.Name!, update) != null ? NoContent() : NotFound();
    }
}
