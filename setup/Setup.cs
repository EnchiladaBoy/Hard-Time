using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace HardTimeSetup
{
    internal static class Program
    {
        [STAThread]
        static int Main(string[] args)
        {
            if (args.SequenceEqual(new[] { "--probe-graphics" }))
            {
                try { return GraphicsProbe.Available() ? 0 : 1; } catch { return 1; }
            }
            if (args.Length != 0) return CommandLine(args);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new SetupWindow());
            return 0;
        }

        static int CommandLine(string[] args)
        {
            string resultFile = null;
            try
            {
                var options = new InstallOptions();
                bool install = false;
                for (int i = 0; i < args.Length; ++i)
                {
                    switch (args[i])
                    {
                        case "--install": install = true; break;
                        case "--download-original": options.Download = true; break;
                        case "--source": options.Source = args[++i]; break;
                        case "--destination": options.Destination = args[++i]; break;
                        case "--renderer": options.Renderer = args[++i]; break;
                        case "--no-shortcuts": options.Shortcuts = false; break;
                        case "--result": resultFile = Path.GetFullPath(args[++i]); break;
                        default: throw new ArgumentException("Unknown argument: " + args[i]);
                    }
                }
                if (!install || (!options.Download && String.IsNullOrWhiteSpace(options.Source)) || String.IsNullOrWhiteSpace(options.Destination))
                    throw new ArgumentException("Use --install (--source PATH | --download-original) --destination PATH [--renderer auto|gpu|software] [--no-shortcuts] [--result FILE].");
                var result = SetupCore.Install(options, delegate { }, CancellationToken.None);
                if (resultFile != null) File.WriteAllText(resultFile, SetupCore.Json.Serialize(result));
                return 0;
            }
            catch (Exception error)
            {
                if (resultFile != null) File.WriteAllText(resultFile, SetupCore.Json.Serialize(new { error = error.Message }));
                return 1;
            }
        }
    }

    internal sealed class SetupWindow : Form
    {
        readonly TextBox source = new TextBox(), destination = new TextBox();
        readonly ComboBox renderer = new ComboBox();
        readonly CheckBox shortcuts = new CheckBox(), downloadOriginal = new CheckBox();
        readonly Button install = new Button(), cancel = new Button(), browseFile = new Button(), browseFolder = new Button(), browseDestination = new Button();
        readonly ProgressBar progress = new ProgressBar();
        readonly Label status = new Label();
        CancellationTokenSource cancellation;
        InstallResult installed;
        bool finishing;
        static readonly Color Background = Color.FromArgb(20, 27, 35), Muted = Color.FromArgb(170, 186, 201), Accent = Color.FromArgb(237, 148, 85);

        internal SetupWindow()
        {
            Text = "Hard Time Setup";
            Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
            ClientSize = new Size(700, 550);
            FormBorderStyle = FormBorderStyle.FixedSingle;
            MaximizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;
            AutoScaleMode = AutoScaleMode.Dpi;
            Font = new Font("Segoe UI", 10);
            BackColor = Background;
            ForeColor = Color.White;
            LabelAt("HARD TIME", 28, 20, 640, 52, 28, Color.White, FontStyle.Bold);
            LabelAt("THE ORIGINAL GAME. READY FOR MODERN WINDOWS.", 30, 77, 640, 23, 10, Accent, FontStyle.Bold);
            LabelAt("Download, install, play. Setup handles the rest.", 30, 112, 640, 25, 11, Muted, FontStyle.Regular);
            LabelAt("1   Original game", 30, 157, 166, 25, 10, Color.White, FontStyle.Bold);
            downloadOriginal.SetBounds(205, 153, 465, 29);
            downloadOriginal.Text = "Download from MDickie automatically";
            downloadOriginal.CheckedChanged += delegate
            {
                source.Enabled = browseFile.Enabled = browseFolder.Enabled = !downloadOriginal.Checked;
            };
            Controls.Add(downloadOriginal);
            Field(source, 30, 188, 424);
            ButtonAt(browseFile, "Installer...", 464, 186, 98, delegate
            {
                using (var dialog = new OpenFileDialog { Title = "Choose the original Hard Time installer", Filter = "Hard Time installer (*.exe)|*.exe", FileName = "hardtime_setup.exe" })
                    if (dialog.ShowDialog(this) == DialogResult.OK) source.Text = dialog.FileName;
            });
            ButtonAt(browseFolder, "Folder...", 572, 186, 98, delegate { PickFolder(source, "Choose the original Hard Time game folder"); });
            var download = new LinkLabel { Text = "Original game by Mat Dickie / MDickie.com", Location = new Point(30, 220), AutoSize = true,
                LinkColor = Muted, ActiveLinkColor = Accent, VisitedLinkColor = Muted };
            download.LinkClicked += delegate { Process.Start(new ProcessStartInfo("https://www.mdickie.com/downloads.htm") { UseShellExecute = true }); };
            Controls.Add(download);
            LabelAt("2   Install to", 30, 260, 640, 25, 10, Color.White, FontStyle.Bold);
            Field(destination, 30, 291, 532);
            ButtonAt(browseDestination, "Change...", 572, 289, 98, delegate { PickFolder(destination, "Choose a separate empty folder or an existing native installation"); });
            destination.Text = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "Hard Time");
            LabelAt("Graphics", 30, 337, 80, 28, 10, Muted, FontStyle.Regular);
            renderer.SetBounds(112, 334, 334, 30);
            renderer.DropDownStyle = ComboBoxStyle.DropDownList;
            renderer.Items.AddRange(new object[] { "Automatic (recommended)", "GPU driver", "Compatibility (software rendering)" });
            renderer.SelectedIndex = 0;
            Controls.Add(renderer);
            shortcuts.Text = "Create Desktop and Start menu shortcuts";
            shortcuts.Checked = true;
            shortcuts.SetBounds(30, 378, 630, 26);
            Controls.Add(shortcuts);
            progress.SetBounds(30, 423, 640, 6);
            Controls.Add(progress);
            status.SetBounds(30, 440, 640, 45);
            status.ForeColor = Muted;
            status.Text = "Your existing native saves and photos are kept when updating.";
            Controls.Add(status);
            ButtonAt(cancel, "Close", 440, 495, 104, delegate
            {
                if (cancellation != null) { cancellation.Cancel(); cancel.Enabled = false; status.Text = "Cancelling..."; }
                else Close();
            });
            ButtonAt(install, "Install game", 554, 495, 116, StartInstall);
            install.BackColor = Accent;
            install.ForeColor = Background;
            install.Font = new Font(Font, FontStyle.Bold);
            AcceptButton = install;
            CancelButton = cancel;
            string candidate = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads", "hardtime_setup.exe");
            if (File.Exists(candidate)) source.Text = candidate;
            else downloadOriginal.Checked = true;
            FormClosing += delegate(object sender, FormClosingEventArgs e)
            {
                if (cancellation == null) return;
                e.Cancel = true;
                if (!finishing) { cancellation.Cancel(); status.Text = "Cancelling..."; }
            };
        }

        void LabelAt(string text, int x, int y, int width, int height, float size, Color color, FontStyle style)
        {
            Controls.Add(new Label { Text = text, Location = new Point(x, y), Size = new Size(width, height),
                ForeColor = color, Font = new Font("Segoe UI", size, style) });
        }

        void Field(TextBox field, int x, int y, int width)
        {
            field.SetBounds(x, y, width, 28);
            field.BackColor = Color.FromArgb(34, 45, 57);
            field.ForeColor = Color.White;
            field.BorderStyle = BorderStyle.FixedSingle;
            Controls.Add(field);
        }

        void ButtonAt(Button button, string text, int x, int y, int width, EventHandler click)
        {
            button.Text = text; button.SetBounds(x, y, width, 32);
            button.FlatStyle = FlatStyle.Flat;
            button.FlatAppearance.BorderColor = Color.FromArgb(77, 95, 112);
            button.Click += click;
            Controls.Add(button);
        }

        void PickFolder(TextBox field, string description)
        {
            using (var dialog = new FolderBrowserDialog { Description = description, SelectedPath = field.Text, ShowNewFolderButton = true })
                if (dialog.ShowDialog(this) == DialogResult.OK) field.Text = dialog.SelectedPath;
        }

        async void StartInstall(object sender, EventArgs e)
        {
            if (installed != null)
            {
                Process.Start(new ProcessStartInfo(Path.Combine(installed.destination, "Play Hard Time.exe")) { WorkingDirectory = installed.destination });
                Close(); return;
            }
            if ((!downloadOriginal.Checked && String.IsNullOrWhiteSpace(source.Text)) || String.IsNullOrWhiteSpace(destination.Text))
            { status.Text = "Choose your original game and an installation folder first."; return; }
            var options = new InstallOptions { Source = source.Text.Trim(), Destination = destination.Text.Trim(),
                Renderer = new[] { "auto", "gpu", "software" }[renderer.SelectedIndex], Shortcuts = shortcuts.Checked, Download = downloadOriginal.Checked };
            cancellation = new CancellationTokenSource();
            finishing = false;
            foreach (Control control in new Control[] { source, destination, renderer, shortcuts, downloadOriginal, browseFile, browseFolder, browseDestination, install }) control.Enabled = false;
            cancel.Text = "Cancel";
            var updates = new Progress<Tuple<int, string>>(update =>
            {
                progress.Value = update.Item1; status.Text = update.Item2;
                if (update.Item1 >= 90) { finishing = true; cancel.Enabled = false; }
            });
            try
            {
                installed = await Task.Run(() => SetupCore.Install(options,
                    (value, text) => ((IProgress<Tuple<int, string>>)updates).Report(Tuple.Create(value, text)), cancellation.Token));
                install.Text = "Play Hard Time";
                status.Text = installed.warning ?? (installed.renderer == "software" ?
                    "Ready to play. Compatibility graphics enabled; crowded scenes may run slowly." : "Ready to play with your graphics driver.");
            }
            catch (OperationCanceledException) { progress.Value = 0; status.Text = "Setup cancelled. You can try again whenever you're ready."; }
            catch (Exception error)
            {
                progress.Value = 0; status.Text = "Setup needs attention. Adjust your choices and try again.";
                MessageBox.Show(this, error.Message, "Hard Time Setup", MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
            finally
            {
                cancellation.Dispose(); cancellation = null;
                cancel.Enabled = true; cancel.Text = "Close"; install.Enabled = true;
                if (installed == null)
                {
                    foreach (Control control in new Control[] { source, destination, renderer, shortcuts, downloadOriginal, browseFile, browseFolder, browseDestination }) control.Enabled = true;
                    source.Enabled = browseFile.Enabled = browseFolder.Enabled = !downloadOriginal.Checked;
                }
            }
        }
    }
}
