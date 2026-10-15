using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;

namespace HardTimeSetup
{
    internal static class Files
    {
        internal const string Marker = ".hardtime-native-install.json";
        internal static readonly string[] Assets = { "Characters", "Graphics", "Sound", "Weapons", "World", "Data" };

        internal static string Full(string path)
        {
            return Path.GetFullPath(path).TrimEnd(Path.DirectorySeparatorChar);
        }

        internal static bool Within(string path, string parent)
        {
            return Full(path).Equals(Full(parent), StringComparison.OrdinalIgnoreCase) ||
                Full(path).StartsWith(Full(parent) + "\\", StringComparison.OrdinalIgnoreCase);
        }

        // Reject redirections before copying, updating, backing up or removing a tree.
        internal static void PrivatePath(string path)
        {
            for (string current = Path.GetFullPath(path); !String.IsNullOrEmpty(current); current = Path.GetDirectoryName(current))
                if ((File.Exists(current) || Directory.Exists(current)) &&
                    (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                    throw new IOException("Choose an ordinary folder, not a link or junction: " + current);
        }

        internal static void CheckTree(string path)
        {
            PrivatePath(path);
            if (Directory.Exists(path))
                foreach (string entry in Directory.EnumerateFileSystemEntries(path)) CheckTree(entry);
        }

        internal static void CopyTree(string source, string destination, Action check)
        {
            PrivatePath(source);
            PrivatePath(destination);
            check();
            Directory.CreateDirectory(destination);
            foreach (string file in Directory.EnumerateFiles(source))
            {
                check();
                PrivatePath(file);
                File.Copy(file, Path.Combine(destination, Path.GetFileName(file)), false);
            }
            foreach (string directory in Directory.EnumerateDirectories(source))
                CopyTree(directory, Path.Combine(destination, Path.GetFileName(directory)), check);
        }

        internal static void RemoveOwnedTree(string path, string parent)
        {
            if (Full(path).Equals(Full(parent), StringComparison.OrdinalIgnoreCase) || !Within(path, parent))
                throw new IOException("Cleanup path is outside its owned folder.");
            if (!Directory.Exists(path)) return;
            CheckTree(path);
            Directory.Delete(path, true);
        }

        internal static FileStream Lock(string root)
        {
            PrivatePath(Path.Combine(root, ".launch.lock"));
            try
            {
                return new FileStream(Path.Combine(root, ".launch.lock"), FileMode.OpenOrCreate,
                    FileAccess.ReadWrite, FileShare.None);
            }
            catch (IOException)
            {
                throw new IOException("Close Hard Time and any other setup window, then try again.");
            }
        }

        internal static void RecoverData(string root)
        {
            string data = Path.Combine(root, "Data"), marker = Path.Combine(root, "Data.transaction");
            PrivatePath(data);
            if (Directory.Exists(data) || !File.Exists(marker)) return;
            PrivatePath(marker);
            string[] lines = File.ReadAllLines(marker);
            if (!lines.SequenceEqual(new[] { "Hard Time save transaction v2", "Data" }))
                throw new IOException("The save recovery marker is unrecognized. Keep this folder intact so your saves can be recovered.");
            string previous = Path.Combine(root, "Data.previous");
            CheckTree(previous);
            if (!Directory.Exists(previous)) throw new IOException("The previous save folder is missing. Setup will not replace your recovery data.");
            Directory.Move(previous, data);
        }

        internal static string Hash(string path)
        {
            using (var sha = SHA256.Create())
            using (var file = File.OpenRead(path))
                return BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant();
        }

        internal static void DurableText(string path, string value)
        {
            byte[] bytes = new System.Text.UTF8Encoding(false).GetBytes(value);
            using (var stream = new FileStream(path, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            {
                stream.Write(bytes, 0, bytes.Length);
                stream.Flush(true);
            }
        }

        internal static string Quote(string value)
        {
            // Windows CommandLineToArgvW quoting; no shell is involved.
            var result = new System.Text.StringBuilder("\"");
            int slashes = 0;
            foreach (char character in value)
            {
                if (character == '\\') { ++slashes; continue; }
                result.Append('\\', character == '"' ? slashes * 2 + 1 : slashes);
                result.Append(character); slashes = 0;
            }
            result.Append('\\', slashes * 2);
            return result.Append('"').ToString();
        }
    }
}
