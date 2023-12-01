﻿namespace Flasher.Host.AOT.Handlers.Cards;

public sealed record CreateCardRequest
{
    public required string Prompt { get; init; }
    public required string Solution { get; init; }
}
