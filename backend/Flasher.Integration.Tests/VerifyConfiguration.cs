using System.Runtime.CompilerServices;

namespace Flasher.Integration.Tests.AutoSave;

public static class VerifyConfiguration
{
    [ModuleInitializer]
    public static void Initialize()
    {
        VerifierSettings.InitializePlugins();
        VerifierSettings.ScrubMembers("__Host-jwt", "jsonWebToken");
    }
}
