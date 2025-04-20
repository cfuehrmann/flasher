using System.Runtime.CompilerServices;

namespace Flasher.Integration.Tests;

public static partial class VerifyConfiguration
{
    [ModuleInitializer]
    public static void Initialize()
    {
        VerifierSettings.InitializePlugins();
        VerifierSettings.ScrubMembers("__Host-jwt", "jsonWebToken");
        VerifierSettings.ScrubInlineGuids();
    }
}
