using System;

namespace Flasher.Store.Exceptions;

public class StoreConfigurationException : Exception
{
    public StoreConfigurationException()
        : base() { }

    public StoreConfigurationException(string message)
        : base(message) { }
}
