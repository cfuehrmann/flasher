﻿using System;

using Flasher.Store.Cards;

namespace Flasher.Store.FileStore.Cards;

public sealed record CachedCard
{
    public CachedCard(
        string id, string prompt, string solution, State state, DateTime changeTime, DateTime nextTime, bool disabled)
    {
        this.Id = id;
        this.Prompt = prompt;
        this.Solution = solution;
        this.State = state;
        this.ChangeTime = changeTime;
        this.NextTime = nextTime;
        this.Disabled = disabled;
    }

    public string Id { get; }
    public string Prompt { get; set; } // intentionally mutable!
    public string Solution { get; set; }
    public State State { get; set; }
    public DateTime ChangeTime { get; set; }
    public DateTime NextTime { get; set; }
    public bool Disabled { get; set; }
}
