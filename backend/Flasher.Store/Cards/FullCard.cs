using System;

namespace Flasher.Store.Cards;

public record FullCard(
    string Id,
    string Prompt,
    string Solution,
    State State,
    DateTime ChangeTime,
    DateTime NextTime,
    bool Disabled);
