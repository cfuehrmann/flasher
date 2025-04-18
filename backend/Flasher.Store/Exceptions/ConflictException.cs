namespace Flasher.Store.Exceptions;

public class ConflictException : Exception
{
    public ConflictException()
        : base() { }

    public ConflictException(string message)
        : base(message) { }
}
