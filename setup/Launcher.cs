using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Windows.Forms;

namespace HardTimeSetup
{
    internal static class Launcher
    {
        [STAThread]
        static int Main()
        {
            Application.EnableVisualStyles();
            string root = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
            try
            {
                Files.PrivatePath(root);
                using (Files.Lock(root))
                {
                    string transaction = Path.Combine(root, ".hardtime-setup");
                    if (Directory.Exists(transaction) && !File.Exists(Path.Combine(transaction, "committed")))
                        throw new IOException("Setup was interrupted. Run Hard-Time-Setup.exe again to finish recovery before playing.");
                    Files.RecoverData(root);
                    string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss-fff") + "-" + Guid.NewGuid().ToString("N").Substring(0, 8);
                    string backups = Path.Combine(root, "backups"), logs = Path.Combine(root, "logs");
                    Files.PrivatePath(backups); Files.PrivatePath(logs);
                    Files.CopyTree(Path.Combine(root, "Data"), Path.Combine(backups, stamp), delegate { });
                    Directory.CreateDirectory(logs);
                    string output = Path.Combine(logs, "run-" + stamp + ".log"), errors = Path.Combine(logs, "run-" + stamp + ".errors.log");
                    var start = new ProcessStartInfo(Path.Combine(root, "Hard Time.exe")) { WorkingDirectory = root,
                        UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
                    if (File.Exists(Path.Combine(root, "software-renderer.txt"))) start.EnvironmentVariables["GALLIUM_DRIVER"] = "llvmpipe";
                    int exit;
                    using (var writer = new StreamWriter(output))
                    using (var errorWriter = new StreamWriter(errors))
                    using (var process = new Process { StartInfo = start })
                    {
                        writer.AutoFlush = true; errorWriter.AutoFlush = true;
                        process.OutputDataReceived += (sender, e) => { if (e.Data != null) writer.WriteLine(e.Data); };
                        process.ErrorDataReceived += (sender, e) => { if (e.Data != null) errorWriter.WriteLine(e.Data); };
                        process.Start(); process.BeginOutputReadLine(); process.BeginErrorReadLine();
                        process.WaitForExit(); exit = process.ExitCode;
                        writer.WriteLine("Exit status: " + exit);
                    }
                    File.WriteAllLines(Path.Combine(logs, "latest.txt"), new[] { output, errors });
                    if (exit != 0) throw new IOException("Hard Time exited unexpectedly. Diagnostics are in:\n" + logs +
                        "\n\nIf the game did not open, rerun setup and choose Compatibility graphics. Your saves will be kept.");
                    return 0;
                }
            }
            catch (Exception error)
            {
                MessageBox.Show(error.Message, "Hard Time", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return 1;
            }
        }
    }
}
