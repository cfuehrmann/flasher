using System.Threading.Tasks;

namespace Flasher.Store.AutoSaving;

public interface IAutoSaveStore
{
  Task<AutoSave?> Read(string user);
  Task Write(string user, AutoSave autoSave);
  Task Delete(string user);
}