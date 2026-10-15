using System;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Threading;
using HardTimeSetup;

internal static class SetupChecks
{
    static int passed;
    static string scratch;
    static void Require(bool value, string message) { if (!value) throw new Exception(message); }
    static void Reject(Action action)
    {
        try { action(); } catch (IOException) { return; }
        throw new Exception("Expected this operation to be refused.");
    }
    static string Folder(string name)
    {
        string result = Path.Combine(scratch, name); Directory.CreateDirectory(result); return result;
    }
    static void Test(string name, Action action)
    {
        action(); ++passed; Console.WriteLine("PASS " + name);
    }
    static Journal PrepareTransaction(string root)
    {
        string transaction = Path.Combine(root, SetupCore.Transaction);
        Directory.CreateDirectory(Path.Combine(transaction, "new/Data"));
        File.WriteAllText(Path.Combine(transaction, "owner"), SetupCore.Owner);
        File.WriteAllText(Path.Combine(root, "Hard Time.exe"), "old game");
        File.WriteAllText(Path.Combine(root, "opengl32.dll"), "old renderer");
        File.WriteAllText(Path.Combine(transaction, "new/Hard Time.exe"), "new game");
        File.WriteAllText(Path.Combine(transaction, "new/Data/fresh.dat"), "new data");
        File.WriteAllText(Path.Combine(transaction, "new/Play Hard Time.exe"), "new launcher");
        return new Journal { operations = {
            new Operation { path = "Hard Time.exe", had_original = true },
            new Operation { path = "opengl32.dll", had_original = true },
            new Operation { path = "Data", had_original = false },
            new Operation { path = "Play Hard Time.exe", had_original = false }
        } };
    }
    [STAThread]
    static int Main(string[] args)
    {
        scratch = Path.GetFullPath(args[0]); Directory.CreateDirectory(scratch);
        try
        {
            for (int boundary = 1; boundary <= 4; ++boundary)
            {
                int stop = boundary;
                Test("rollback interrupted update after move " + stop, delegate
                {
                    string root = Folder("interrupted-" + stop);
                    var journal = PrepareTransaction(root);
                    try { SetupCore.Commit(root, journal, index => { if (index == stop) throw new IOException("simulated power loss"); }); }
                    catch (IOException) { }
                    SetupCore.ValidateDestination(root, Path.Combine(scratch, "source"));
                    SetupCore.RecoverSetup(root); SetupCore.RecoverSetup(root);
                    Require(File.ReadAllText(Path.Combine(root, "Hard Time.exe")) == "old game", "Old program was not restored.");
                    Require(File.ReadAllText(Path.Combine(root, "opengl32.dll")) == "old renderer", "Deleted renderer was not restored.");
                    Require(!Directory.Exists(Path.Combine(root, "Data")), "New data survived rollback.");
                    Require(!File.Exists(Path.Combine(root, "Play Hard Time.exe")), "New launcher survived rollback.");
                });
            }
            Test("committed update survives interrupted cleanup", delegate
            {
                string root = Folder("committed"); var journal = PrepareTransaction(root);
                SetupCore.Commit(root, journal, delegate { }); SetupCore.RecoverSetup(root);
                Require(File.ReadAllText(Path.Combine(root, "Hard Time.exe")) == "new game", "Committed program reverted.");
                Require(!File.Exists(Path.Combine(root, "opengl32.dll")), "Old renderer survived.");
                Require(File.Exists(Path.Combine(root, "Data/fresh.dat")), "Committed data disappeared.");
            });
            Test("unfinished staging leaves existing saves intact", delegate
            {
                string root = Folder("cancelled"), transaction = Path.Combine(root, SetupCore.Transaction);
                Directory.CreateDirectory(Path.Combine(root, "Data")); File.WriteAllText(Path.Combine(root, "Data/save.dat"), "my save");
                Directory.CreateDirectory(Path.Combine(transaction, "new")); File.WriteAllText(Path.Combine(transaction, "owner"), SetupCore.Owner);
                SetupCore.RecoverSetup(root);
                Require(File.ReadAllText(Path.Combine(root, "Data/save.dat")) == "my save", "Existing save changed.");
            });
            Test("launcher and updater cannot share a live installation", delegate
            {
                string root = Folder("lock");
                using (Files.Lock(root)) Reject(delegate { using (Files.Lock(root)) { } });
            });
            Test("a directly opened game executable also excludes setup", delegate
            {
                string root = Folder("direct-game"), binary = Path.Combine(root, "Hard Time.exe");
                File.WriteAllText(binary, "game");
                using (new FileStream(binary, FileMode.Open, FileAccess.Read, FileShare.Read))
                    Reject(delegate { SetupCore.EnsureGameClosed(root); });
                SetupCore.EnsureGameClosed(root);
            });
            Test("recognize and recover interrupted save directory switch", delegate
            {
                string root = Folder("save-recovery"); Directory.CreateDirectory(Path.Combine(root, "Data.previous"));
                File.WriteAllText(Path.Combine(root, "Data.previous/player.dat"), "player");
                File.WriteAllText(Path.Combine(root, "Data.transaction"), "Hard Time save transaction v2\nData\n");
                Files.RecoverData(root);
                Require(File.ReadAllText(Path.Combine(root, "Data/player.dat")) == "player", "Recovered data differs.");
            });
            Test("unrecognized save marker cannot be replaced with fresh data", delegate
            {
                string root = Folder("bad-marker"); File.WriteAllText(Path.Combine(root, "Data.transaction"), "other game\nData\n");
                Reject(delegate { Files.RecoverData(root); });
            });
            Test("source and destination may not overlap", delegate
            {
                string root = Folder("overlap");
                Reject(delegate { SetupCore.ValidateDestination(root, root); });
                Reject(delegate { SetupCore.ValidateDestination(root, Path.Combine(root, "original")); });
                Reject(delegate { SetupCore.ValidateDestination(Path.Combine(root, "native"), root); });
            });
            Test("unrelated destination is preserved", delegate
            {
                string root = Folder("unrelated"); File.WriteAllText(Path.Combine(root, "important.txt"), "keep");
                Reject(delegate { SetupCore.ValidateDestination(root, Path.Combine(scratch, "original")); });
                Require(File.ReadAllText(Path.Combine(root, "important.txt")) == "keep", "Unrelated file changed.");
            });
            Test("archive paths cannot escape the temporary workspace", delegate
            {
                foreach (string name in new[] { "../escape", "C:/escape", "/escape", "runtime/../../escape", "runtime\\..\\escape", "runtime/file:stream" })
                    using (var buffer = new MemoryStream())
                    {
                        using (var zip = new ZipArchive(buffer, ZipArchiveMode.Create, true))
                            using (var writer = new StreamWriter(zip.CreateEntry(name).Open())) writer.Write("unexpected");
                        buffer.Position = 0;
                        Reject(delegate { SetupCore.Unpack(buffer, Folder("zip"), CancellationToken.None); });
                    }
            });
            Test("recovery journals cannot replace existing player data", delegate
            {
                string root = Folder("journal-protection"), transaction = Path.Combine(root, SetupCore.Transaction);
                Directory.CreateDirectory(transaction); File.WriteAllText(Path.Combine(transaction, "owner"), SetupCore.Owner);
                File.WriteAllText(Path.Combine(transaction, "journal.json"), "{\"schema\":1,\"operations\":[{\"path\":\"Data\",\"had_original\":true}]}");
                Reject(delegate { SetupCore.RecoverSetup(root); });
            });
            Test("cancellation stops unpacking before installation", delegate
            {
                using (var buffer = new MemoryStream())
                {
                    using (var zip = new ZipArchive(buffer, ZipArchiveMode.Create, true)) zip.CreateEntry("file");
                    buffer.Position = 0;
                    try { SetupCore.Unpack(buffer, Folder("cancel-unpack"), new CancellationToken(true)); throw new Exception("Cancellation ignored."); }
                    catch (OperationCanceledException) { }
                }
            });
            Test("shortcuts update in place and cannot overwrite another installation", delegate
            {
                string root = Folder("shortcut-install"), desktop = Folder("desktop"), programs = Folder("programs");
                SetupCore.CreateShortcuts(root, new[] { desktop, programs });
                SetupCore.CreateShortcuts(root, new[] { desktop, programs });
                Require(Directory.GetFiles(desktop).Length == 1 && Directory.GetFiles(programs).Length == 1, "Update created duplicate shortcuts.");
                string previous = Files.Hash(Path.Combine(desktop, "Hard Time (Native).lnk"));
                Reject(delegate { SetupCore.CreateShortcuts(Folder("other-install"), new[] { desktop }); });
                Require(Files.Hash(Path.Combine(desktop, "Hard Time (Native).lnk")) == previous, "A different installation's shortcut was overwritten.");
            });
            Console.WriteLine(passed + " setup checks passed."); return 0;
        }
        catch (Exception error) { Console.Error.WriteLine(error); return 1; }
    }
}
