using System.Text.Json;
using System.Text.Json.Serialization;

namespace Flasher.Store.FileStore;

public class FileStoreJsonContextProvider : IFileStoreJsonContextProvider
{
    public JsonSerializerContext Instance
    {
        get
        {
            var jsonOptions = new JsonSerializerOptions { WriteIndented = true, PropertyNameCaseInsensitive = true };
            jsonOptions.Converters.Add(new JsonStringEnumConverter());
            return new FileStoreJsonContext(jsonOptions);
        }
    }
}
