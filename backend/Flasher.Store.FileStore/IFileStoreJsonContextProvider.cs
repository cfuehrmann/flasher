using System.Text.Json.Serialization;

namespace Flasher.Store.FileStore;

public interface IFileStoreJsonContextProvider
{
    public JsonSerializerContext Instance { get; }
}
