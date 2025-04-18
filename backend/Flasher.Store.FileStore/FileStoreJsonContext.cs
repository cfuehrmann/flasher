using System.Text.Json.Serialization;
using Flasher.Store.FileStore.Authentication;
using Flasher.Store.FileStore.AutoSaving;
using Flasher.Store.FileStore.Cards;

namespace Flasher.Store.FileStore;

[JsonSerializable(typeof(SerializableCard))]
[JsonSerializable(typeof(IEnumerable<SerializableCard>))]
[JsonSerializable(typeof(CachedCard))]
[JsonSerializable(typeof(IEnumerable<CachedCard>))]
[JsonSerializable(typeof(SerializableAutoSave))]
[JsonSerializable(typeof(Profile))]
public partial class FileStoreJsonContext : JsonSerializerContext { }
