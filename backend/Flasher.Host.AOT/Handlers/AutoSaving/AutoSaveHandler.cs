using Flasher.Store.AutoSaving;

namespace Flasher.Host.AOT.Handlers.AutoSaving;

public static class AutoSaveHandler
{
    public static async Task Delete(HttpContext context, IAutoSaveStore store)
    {
        await store.Delete(context.User.Identity!.Name!);
    }

    public static async Task Write(
        WriteAutoSaveRequest request,
        HttpContext context,
        IAutoSaveStore store
    )
    {
        await store.Write(
            context.User.Identity!.Name!,
            new AutoSave
            {
                Id = request.Id,
                Prompt = request.Prompt,
                Solution = request.Solution
            }
        );
    }
}
