using System.Threading.Tasks;

namespace Flasher.Store.Cards
{
    public interface ICardStore
    {
        Task Create(string user, FullCard card);
        Task<FullCard?> Read(string user, string id);
        Task<bool> Update(string user, CardUpdate update);
        Task<bool> Delete(string user, string id);
        Task<FindResponse> Find(string user, string searchText);
        Task<FullCard?> FindNext(string user);
    }
}