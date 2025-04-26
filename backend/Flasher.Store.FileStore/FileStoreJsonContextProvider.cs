using System.Text.Json;
using System.Text.Json.Serialization;

namespace Flasher.Store.FileStore;

public class FileStoreJsonContextProvider : IFileStoreJsonContextProvider
{
    public JsonSerializerContext Instance
    {
        get
        {
            var jsonOptions = new JsonSerializerOptions
            {
                WriteIndented = true,
                PropertyNameCaseInsensitive = true,
            };
            return new FileStoreJsonContext(jsonOptions);
        }
    }
}
