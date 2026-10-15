using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Net;
using System.Reflection;
using System.Threading;
using System.Web.Script.Serialization;

namespace HardTimeSetup
{
    internal sealed class InstallOptions
    {
        public string Source, Destination, Renderer = "auto";
        public bool Shortcuts = true, Download;
    }

    internal sealed class InstallResult
    {
        public string destination, renderer, warning;
    }

    internal sealed class InstallMarker
    {
        public string platform, executable_sha256;
        public bool software_renderer;
    }

    internal sealed class PayloadManifest
    {
        public int schema { get; set; }
        public string game_sha256 { get; set; }
        public Dictionary<string, string> files { get; set; }
    }

    internal sealed class Operation
    {
        public string path;
        public bool had_original;
    }

    internal sealed class Journal
    {
        public int schema = 1;
        public List<Operation> operations = new List<Operation>();
    }

    internal static class SetupCore
    {
        internal const string Transaction = ".hardtime-setup";
        internal const string Owner = "Hard Time setup transaction v1\n";
        internal const string OriginalHash = "72d4f6a2e9c22a77f63b2f90730cb71bb742ecf9c1782000f9a5e4cb641c6ca1";
        internal const string OriginalUrl = "https://www.mdickie.com/downloads/new/hardtime_setup.exe";
        internal static readonly JavaScriptSerializer Json = new JavaScriptSerializer();
        internal static readonly string[] Managed = { "Hard Time.exe", "Play Hard Time.exe", "OpenAL32.dll",
            "opengl32.dll", "libgallium_wgl.dll", "software-renderer.txt", "Read Me.txt", "NOTICE.txt",
            "play.ps1", "Play Hard Time.cmd", Files.Marker };

        internal static void ValidateAssets(string source)
        {
            Files.PrivatePath(source);
            foreach (string name in Files.Assets)
                if (!Directory.Exists(Path.Combine(source, name)) ||
                    !Directory.EnumerateFileSystemEntries(Path.Combine(source, name)).Any())
                    throw new IOException("The original game folder is missing " + name + ". Select hardtime_setup.exe or the folder containing the complete original game.");
            foreach (string name in new[] { "Graphics/Logo01.PNG", "Data/Options.dat", "Data/Structure.dat" })
                if (!File.Exists(Path.Combine(source, name))) throw new IOException("This does not look like a complete Hard Time (2007) installation: " + name + " is missing.");
        }

        internal static void ValidateDestination(string destination, string source)
        {
            if (destination.StartsWith(@"\\") || destination.Length > 150 || Path.GetPathRoot(destination).TrimEnd('\\') == destination)
                throw new IOException("Choose a short, local installation path, such as the suggested folder.");
            Files.PrivatePath(destination);
            if (Files.Within(destination, source) || Files.Within(source, destination))
                throw new IOException("Choose a separate destination so the original game stays intact.");
            if (File.Exists(destination)) throw new IOException("The destination is a file. Choose a folder.");
            string marker = Path.Combine(destination, Files.Marker);
            Files.PrivatePath(marker);
            if (File.Exists(marker))
            {
                var old = Json.Deserialize<InstallMarker>(File.ReadAllText(marker));
                if (old == null || old.platform != "windows-x64" || String.IsNullOrEmpty(old.executable_sha256))
                    throw new IOException("This folder is not a recognized native Hard Time installation.");
            }
            else if (Directory.Exists(destination) && Directory.EnumerateFileSystemEntries(destination)
                .Any(p => Path.GetFileName(p) != ".launch.lock" && Path.GetFileName(p) != Transaction))
            {
                string owner = Path.Combine(destination, Transaction, "owner");
                Files.PrivatePath(owner);
                if (!File.Exists(owner) || File.ReadAllText(owner) != Owner)
                    throw new IOException("That folder already contains other files. Choose an empty folder or an existing native Hard Time installation.");
            }
        }

        internal static string EntryPath(string root, string relative)
        {
            if (String.IsNullOrEmpty(relative) || relative.Contains(":") || relative.Contains("\\") ||
                relative.StartsWith("/") || relative.Split('/').Any(p => p == ".." || p == "." || p.Length == 0))
                throw new IOException("Invalid path in the setup package.");
            string path = Path.GetFullPath(Path.Combine(root, relative));
            if (!Files.Within(path, root) || Files.Full(path) == Files.Full(root)) throw new IOException("Invalid package path.");
            return path;
        }

        internal static void Unpack(Stream source, string root, CancellationToken cancel)
        {
            using (var zip = new ZipArchive(source, ZipArchiveMode.Read))
                foreach (var entry in zip.Entries)
                {
                    cancel.ThrowIfCancellationRequested();
                    string target = EntryPath(root, entry.FullName);
                    Directory.CreateDirectory(Path.GetDirectoryName(target));
                    using (var input = entry.Open())
                    using (var output = new FileStream(target, FileMode.CreateNew, FileAccess.Write)) input.CopyTo(output);
                }
            var manifest = Json.Deserialize<PayloadManifest>(File.ReadAllText(Path.Combine(root, "manifest.json")));
            if (manifest == null || manifest.schema != 1 || manifest.files == null) throw new IOException("Invalid setup manifest. Download setup again.");
            foreach (var item in manifest.files)
            {
                cancel.ThrowIfCancellationRequested();
                if (Files.Hash(EntryPath(root, item.Key)) != item.Value) throw new IOException("Setup's bundled files are damaged. Download setup again.");
            }
            if (manifest.game_sha256 != Files.Hash(Path.Combine(root, "runtime/Hard Time.exe")))
                throw new IOException("The bundled game checksum does not match.");
        }

        internal static string ExtractOriginal(string installer, string workspace, CancellationToken cancel)
        {
            if (Files.Hash(installer) != OriginalHash)
                throw new IOException("This installer differs from the tested Hard Time (2007) installer. Choose a complete extracted game folder instead, or get the original from MDickie's website.");
            string output = Path.Combine(workspace, "original");
            Directory.CreateDirectory(output);
            var start = new ProcessStartInfo(Path.Combine(workspace, "tools/7z.exe"),
                "x " + Files.Quote(installer) + " " + Files.Quote("-o" + output) + " -y -bso0 -bse0 -bsp0")
            { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = workspace };
            using (var process = Process.Start(start))
            {
                var timer = Stopwatch.StartNew();
                while (!process.WaitForExit(100))
                    if (cancel.IsCancellationRequested || timer.Elapsed.TotalMinutes > 3)
                    {
                        process.Kill(); process.WaitForExit();
                        cancel.ThrowIfCancellationRequested();
                        throw new IOException("Extracting the original game timed out. Try selecting an extracted game folder.");
                    }
                if (process.ExitCode != 0) throw new IOException("The original installer could not be extracted. Select an extracted game folder instead.");
            }
            return output;
        }

        internal static void DownloadOriginal(string target, Action<int, string> progress, CancellationToken cancel)
        {
            ServicePointManager.SecurityProtocol = SecurityProtocolType.Tls12;
            var request = (HttpWebRequest)WebRequest.Create(OriginalUrl);
            request.UserAgent = "Hard-Time-Native-Setup/1.0";
            request.Timeout = 15000; request.ReadWriteTimeout = 15000;
            // Cancellation aborts a blocked network read as well as the copy loop.
            using (cancel.Register(request.Abort))
            {
                try
                {
                    using (var response = request.GetResponse())
                    using (var input = response.GetResponseStream())
                    using (var output = new FileStream(target, FileMode.CreateNew, FileAccess.Write))
                    {
                        byte[] buffer = new byte[65536]; int count; long total = 0;
                        while ((count = input.Read(buffer, 0, buffer.Length)) > 0)
                        {
                            cancel.ThrowIfCancellationRequested(); total += count;
                            if (total > 32 * 1024 * 1024) throw new IOException("The original download is larger than expected. Choose a local original copy instead.");
                            output.Write(buffer, 0, count);
                            progress(10 + (int)Math.Min(14, total * 14 / 20591920), "Downloading the original game from MDickie... " + total / 1048576 + " MB");
                        }
                    }
                }
                catch (WebException)
                {
                    cancel.ThrowIfCancellationRequested();
                    throw new IOException("The original game could not be downloaded. Check your connection, or turn off automatic download and choose a local original copy.");
                }
            }
            if (Files.Hash(target) != OriginalHash)
                throw new IOException("The downloaded original differs from the tested version. No game files were installed. Choose a complete original game folder instead.");
        }

        internal static bool Allowed(string relative, bool hadOriginal)
        {
            // Recovery journals may only change files owned by setup. Player folders
            // are allowed solely as newly created entries, never as replacements.
            if (Managed.Contains(relative)) return true;
            if (relative.StartsWith("licenses/") && relative.Split('/').Length == 2) return true;
            return !hadOriginal && (Files.Assets.Contains(relative) || relative == "Photo Album" ||
                relative == "Instruction Manual.txt" || relative == "Survival Guide.txt");
        }

        internal static bool Exists(string path) { return File.Exists(path) || Directory.Exists(path); }

        internal static void RemoveEntry(string path, string root)
        {
            Files.PrivatePath(path);
            if (Directory.Exists(path)) Files.RemoveOwnedTree(path, root);
            else if (File.Exists(path)) File.Delete(path);
        }

        internal static void MoveEntry(string source, string target)
        {
            Directory.CreateDirectory(Path.GetDirectoryName(target));
            if (Directory.Exists(source)) Directory.Move(source, target);
            else File.Move(source, target);
        }

        internal static void RecoverSetup(string destination)
        {
            string transaction = Path.Combine(destination, Transaction);
            if (!Directory.Exists(transaction)) return;
            Files.CheckTree(transaction);
            if (!Directory.EnumerateFileSystemEntries(transaction).Any()) { Directory.Delete(transaction); return; }
            if (!File.Exists(Path.Combine(transaction, "owner")) || File.ReadAllText(Path.Combine(transaction, "owner")) != Owner)
                throw new IOException("The setup recovery folder is unrecognized. Keep it intact and choose a new installation folder.");
            string journalPath = Path.Combine(transaction, "journal.json");
            if (File.Exists(journalPath) && !File.Exists(Path.Combine(transaction, "committed")))
            {
                var journal = Json.Deserialize<Journal>(File.ReadAllText(journalPath));
                if (journal == null || journal.schema != 1 || journal.operations == null) throw new IOException("Invalid setup recovery journal.");
                foreach (var item in journal.operations)
                {
                    EntryPath(destination, item.path);
                    if (!Allowed(item.path, item.had_original)) throw new IOException("Unexpected path in setup recovery journal.");
                    Files.PrivatePath(EntryPath(destination, item.path));
                }
                foreach (var item in journal.operations.AsEnumerable().Reverse())
                {
                    string target = EntryPath(destination, item.path), backup = EntryPath(Path.Combine(transaction, "old"), item.path);
                    string staged = EntryPath(Path.Combine(transaction, "new"), item.path);
                    if (item.had_original && File.Exists(backup))
                    {
                        if (Directory.Exists(target)) throw new IOException("A folder is blocking setup recovery: " + target);
                        if (File.Exists(target)) File.Delete(target);
                        MoveEntry(backup, target);
                    }
                    else if (!item.had_original && !Exists(staged)) RemoveEntry(target, destination);
                }
            }
            else if (!File.Exists(journalPath) && Directory.Exists(Path.Combine(transaction, "old")))
                throw new IOException("Setup recovery data has no journal. Keep this folder intact.");
            Files.RemoveOwnedTree(transaction, destination);
        }

        internal static void Commit(string destination, Journal journal, Action<int> afterMove)
        {
            string transaction = Path.Combine(destination, Transaction);
            Files.DurableText(Path.Combine(transaction, "journal.tmp"), Json.Serialize(journal));
            File.Move(Path.Combine(transaction, "journal.tmp"), Path.Combine(transaction, "journal.json"));
            int index = 0;
            foreach (var item in journal.operations)
            {
                string target = EntryPath(destination, item.path);
                string staged = EntryPath(Path.Combine(transaction, "new"), item.path);
                if (item.had_original) MoveEntry(target, EntryPath(Path.Combine(transaction, "old"), item.path));
                if (Exists(staged)) MoveEntry(staged, target);
                afterMove(++index);
            }
            Files.DurableText(Path.Combine(transaction, "committed"), "complete\n");
        }

        internal static bool SoftwareRenderer(string choice, string destination)
        {
            if (choice == "software") return true;
            if (choice == "gpu") return false;
            if (choice != "auto") throw new ArgumentException("Renderer must be auto, gpu or software.");
            string marker = Path.Combine(destination, Files.Marker);
            if (File.Exists(marker)) return Json.Deserialize<InstallMarker>(File.ReadAllText(marker)).software_renderer;
            // Probe in a separate process so a faulty system graphics driver cannot
            // take down setup. Any failure or timeout chooses bundled Mesa.
            var start = new ProcessStartInfo(Assembly.GetExecutingAssembly().Location, "--probe-graphics")
            { UseShellExecute = false, CreateNoWindow = true };
            using (var probe = Process.Start(start))
            {
                if (probe.WaitForExit(10000)) return probe.ExitCode != 0;
                probe.Kill(); probe.WaitForExit(); return true;
            }
        }

        internal static void EnsureGameClosed(string destination)
        {
            string binary = Path.Combine(destination, "Hard Time.exe");
            Files.PrivatePath(binary);
            if (!File.Exists(binary)) return;
            try { using (new FileStream(binary, FileMode.Open, FileAccess.ReadWrite, FileShare.None)) { } }
            catch (IOException) { throw new IOException("Close Hard Time before updating, including any game started directly from Hard Time.exe."); }
        }

        internal static void StageFile(string source, string relative, string destination, Journal journal)
        {
            string target = EntryPath(destination, relative);
            Files.PrivatePath(target);
            if (Directory.Exists(target)) throw new IOException("A folder is blocking this game file: " + target);
            string staged = EntryPath(Path.Combine(destination, Transaction, "new"), relative);
            Directory.CreateDirectory(Path.GetDirectoryName(staged));
            if (source != null) File.Copy(source, staged, false);
            journal.operations.Add(new Operation { path = relative, had_original = File.Exists(target) });
        }

        internal static InstallResult Install(InstallOptions options, Action<int, string> progress, CancellationToken cancel)
        {
            string workspace = Path.Combine(Path.GetTempPath(), "HardTime-Setup-" + Guid.NewGuid().ToString("N"));
            string inputSource = options.Download ? Path.Combine(workspace, "hardtime_setup.exe") : Files.Full(options.Source);
            string source = inputSource, destination = Files.Full(options.Destination);
            ValidateDestination(destination, inputSource);
            if (!options.Download && !Exists(source)) throw new IOException("Choose the original hardtime_setup.exe or the original game folder.");
            Files.PrivatePath(workspace);
            Directory.CreateDirectory(workspace);
            try
            {
                progress(5, "Checking the bundled game and support files...");
                using (var resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("payload.zip"))
                {
                    if (resource == null) throw new IOException("The setup payload is missing.");
                    Unpack(resource, workspace, cancel);
                }
                if (options.Download) DownloadOriginal(source, progress, cancel);
                if (File.Exists(source))
                {
                    progress(25, "Extracting the original game...");
                    source = ExtractOriginal(source, workspace, cancel);
                }
                ValidateAssets(source);
                bool software;
                cancel.ThrowIfCancellationRequested();
                Directory.CreateDirectory(destination);
                using (Files.Lock(destination))
                {
                    // Finish an interrupted setup before examining the installed save data.
                    EnsureGameClosed(destination);
                    RecoverSetup(destination);
                    ValidateDestination(destination, inputSource);
                    Files.RecoverData(destination);
                    progress(40, "Checking graphics support...");
                    software = SoftwareRenderer(options.Renderer, destination);
                    string transaction = Path.Combine(destination, Transaction);
                    Directory.CreateDirectory(transaction);
                    Files.DurableText(Path.Combine(transaction, "owner"), Owner);
                    try
                    {
                        var journal = new Journal();
                        string runtime = Path.Combine(workspace, "runtime");
                        foreach (string file in Directory.EnumerateFiles(runtime, "*", SearchOption.AllDirectories))
                        {
                            cancel.ThrowIfCancellationRequested();
                            string relative = file.Substring(runtime.Length + 1).Replace('\\', '/');
                            if (!Allowed(relative, true)) throw new IOException("Unknown bundled game file: " + relative);
                            StageFile(file, relative, destination, journal);
                        }
                        foreach (string name in new[] { "opengl32.dll", "libgallium_wgl.dll", "software-renderer.txt" })
                            if (software || File.Exists(Path.Combine(destination, name)))
                                StageFile(software ? Path.Combine(workspace, "software", name) : null, name, destination, journal);
                        progress(50, "Copying game files. Existing saves and photos are kept...");
                        foreach (string name in Files.Assets.Concat(new[] { "Photo Album" }))
                        {
                            string target = Path.Combine(destination, name);
                            Files.PrivatePath(target);
                            if (File.Exists(target)) throw new IOException("A file is blocking the game folder: " + target);
                            if (Directory.Exists(target)) continue;
                            string staged = Path.Combine(transaction, "new", name);
                            if (name == "Photo Album") Directory.CreateDirectory(staged);
                            else Files.CopyTree(Path.Combine(source, name), staged, cancel.ThrowIfCancellationRequested);
                            journal.operations.Add(new Operation { path = name });
                        }
                        foreach (string name in new[] { "Instruction Manual.txt", "Survival Guide.txt" })
                            if (File.Exists(Path.Combine(source, name)) && !Exists(Path.Combine(destination, name)))
                                StageFile(Path.Combine(source, name), name, destination, journal);
                        var marker = new InstallMarker { platform = "windows-x64", software_renderer = software,
                            executable_sha256 = Files.Hash(Path.Combine(runtime, "Hard Time.exe")) };
                        string markerFile = Path.Combine(workspace, "install-marker.json");
                        File.WriteAllText(markerFile, Json.Serialize(marker));
                        StageFile(markerFile, Files.Marker, destination, journal);
                        cancel.ThrowIfCancellationRequested();
                        progress(90, "Finishing installation...");
                        Commit(destination, journal, delegate { });
                    }
                    catch
                    {
                        RecoverSetup(destination);
                        throw;
                    }
                    // A committed marker makes cleanup restartable without reverting a
                    // successful install, even if antivirus temporarily holds a file.
                    try { RecoverSetup(destination); } catch (IOException) { }
                }
                string warning = null;
                if (options.Shortcuts)
                {
                    try { CreateShortcuts(destination); }
                    catch (Exception) { warning = "The game is installed. Windows could not create a shortcut; open Play Hard Time.exe in the installation folder."; }
                }
                progress(100, "Ready to play.");
                return new InstallResult { destination = destination, renderer = software ? "software" : "gpu", warning = warning };
            }
            finally
            {
                try { Files.RemoveOwnedTree(workspace, Path.GetTempPath()); } catch (IOException) { }
            }
        }

        internal static void CreateShortcuts(string destination, string[] folders = null)
        {
            if (folders == null) folders = new[] { Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory),
                Environment.GetFolderPath(Environment.SpecialFolder.Programs) };
            Type type = Type.GetTypeFromProgID("WScript.Shell", true);
            object shell = Activator.CreateInstance(type);
            try
            {
                foreach (string folder in folders)
                {
                    string path = Path.Combine(folder, "Hard Time (Native).lnk");
                    Files.PrivatePath(path);
                    object shortcut = type.InvokeMember("CreateShortcut", BindingFlags.InvokeMethod, null, shell, new object[] { path });
                    try
                    {
                        Type st = shortcut.GetType();
                        string previous = (string)st.InvokeMember("TargetPath", BindingFlags.GetProperty, null, shortcut, null);
                        if (File.Exists(path) && !String.Equals(previous, Path.Combine(destination, "Play Hard Time.exe"), StringComparison.OrdinalIgnoreCase))
                            throw new IOException("A shortcut already belongs to a different installation.");
                        st.InvokeMember("TargetPath", BindingFlags.SetProperty, null, shortcut, new object[] { Path.Combine(destination, "Play Hard Time.exe") });
                        st.InvokeMember("WorkingDirectory", BindingFlags.SetProperty, null, shortcut, new object[] { destination });
                        st.InvokeMember("Description", BindingFlags.SetProperty, null, shortcut, new object[] { "Hard Time - native Windows edition" });
                        st.InvokeMember("Save", BindingFlags.InvokeMethod, null, shortcut, null);
                    }
                    finally { System.Runtime.InteropServices.Marshal.ReleaseComObject(shortcut); }
                }
            }
            finally { System.Runtime.InteropServices.Marshal.ReleaseComObject(shell); }
        }
    }
}
