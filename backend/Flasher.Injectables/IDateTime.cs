using System;

namespace Flasher.Injectables;

public interface IDateTime
{
  public DateTime Now { get; }
}