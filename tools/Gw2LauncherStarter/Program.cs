using System;
using System.Diagnostics;

namespace Gw2LauncherStarter
{
    class Program
    {
        [STAThread]
        static void Main(string[] args)
        {
            // Launch GW2Launcher in STA mode
            // This fixes the "DragDrop registration did not succeed" error
            // when GW2Launcher is launched from InnerSpace or other non-STA contexts

            string gw2Path = @"C:\Program Files\Guild Wars 2\Gw2Launcher.exe";

            if (args.Length > 0)
            {
                gw2Path = args[0];
            }

            Console.WriteLine($"Launching {gw2Path} in STA mode...");

            var psi = new ProcessStartInfo
            {
                FileName = gw2Path,
                UseShellExecute = false,
                WorkingDirectory = System.IO.Path.GetDirectoryName(gw2Path)
            };

            var process = Process.Start(psi);
            if (process != null)
            {
                Console.WriteLine($"Launched with PID: {process.Id}");
                process.WaitForExit();
            }
            else
            {
                Console.WriteLine("Failed to launch GW2Launcher");
            }
        }
    }
}
