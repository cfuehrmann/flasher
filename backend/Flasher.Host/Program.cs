using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace Flasher.Host;

public class Program
{
    public static void Main(string[] args)
    {
        CreateHostBuilder(args).Build().Run();
    }

    public static IHostBuilder CreateHostBuilder(string[] args) =>
        Microsoft.Extensions.Hosting.Host.CreateDefaultBuilder(args)
            .ConfigureAppConfiguration((hostingContext, config) =>
                _ = config.AddEnvironmentVariables(prefix: "Flasher_"))
            .ConfigureLogging(logging =>
                _ = logging.ClearProviders().AddConsole().SetMinimumLevel(LogLevel.Information)
            )
            .ConfigureWebHostDefaults(webBuilder => _ = webBuilder.UseStartup<Startup>());
}
