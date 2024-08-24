using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Extensions.Logging;

namespace Flasher.Integration.Tests;

public class TestLogger : ILogger
{
    private readonly ConcurrentQueue<Entry> _entries = new();

    public IReadOnlyList<Entry> Entries => [.. _entries];

    public IDisposable? BeginScope<TState>(TState state)
        where TState : notnull
    {
        return null;
    }

    public bool IsEnabled(LogLevel logLevel)
    {
        return true;
    }

    public void Log<TState>(
        LogLevel logLevel,
        EventId eventId,
        TState state,
        Exception? exception,
        Func<TState, Exception?, string> formatter
    )
    {
        _ = _entries.Append(
            new Entry
            {
                LogLevel = logLevel,
                EventId = eventId,
                Exception = exception,
                Message = formatter(state, exception),
            }
        );
    }

    public sealed record Entry
    {
        public required LogLevel LogLevel { get; init; }
        public required EventId EventId { get; init; }
        public required Exception? Exception { get; init; }
        public required string Message { get; init; }
    }
}
