using System;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace HardTimeSetup
{
    internal static class GraphicsProbe
    {
        [StructLayout(LayoutKind.Sequential)]
        struct PixelFormat
        {
            public ushort size, version;
            public uint flags;
            public byte type, colorBits, redBits, redShift, greenBits, greenShift, blueBits, blueShift,
                alphaBits, alphaShift, accumBits, accumRed, accumGreen, accumBlue, accumAlpha,
                depthBits, stencilBits, auxBuffers, layerType, reserved;
            public uint layerMask, visibleMask, damageMask;
        }
        [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr window);
        [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr window, IntPtr dc);
        [DllImport("gdi32.dll")] static extern int ChoosePixelFormat(IntPtr dc, ref PixelFormat format);
        [DllImport("gdi32.dll")] static extern bool SetPixelFormat(IntPtr dc, int index, ref PixelFormat format);
        [DllImport("opengl32.dll"), DefaultDllImportSearchPaths(DllImportSearchPath.System32)] static extern IntPtr wglCreateContext(IntPtr dc);
        [DllImport("opengl32.dll"), DefaultDllImportSearchPaths(DllImportSearchPath.System32)] static extern bool wglMakeCurrent(IntPtr dc, IntPtr context);
        [DllImport("opengl32.dll"), DefaultDllImportSearchPaths(DllImportSearchPath.System32)] static extern bool wglDeleteContext(IntPtr context);
        [DllImport("opengl32.dll"), DefaultDllImportSearchPaths(DllImportSearchPath.System32)] static extern IntPtr glGetString(uint name);

        internal static bool Available()
        {
            using (var window = new Form())
            {
                IntPtr dc = GetDC(window.Handle), context = IntPtr.Zero;
                try
                {
                    var format = new PixelFormat { size = (ushort)Marshal.SizeOf(typeof(PixelFormat)), version = 1,
                        flags = 0x25, colorBits = 32, depthBits = 24, stencilBits = 8 };
                    int index = ChoosePixelFormat(dc, ref format);
                    if (index == 0 || !SetPixelFormat(dc, index, ref format)) return false;
                    context = wglCreateContext(dc);
                    if (context == IntPtr.Zero || !wglMakeCurrent(dc, context)) return false;
                    string version = Marshal.PtrToStringAnsi(glGetString(0x1F02));
                    double number;
                    return version != null && Double.TryParse(version.Split(' ')[0].Substring(0, 3),
                        NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out number) && number >= 3.3;
                }
                finally
                {
                    wglMakeCurrent(IntPtr.Zero, IntPtr.Zero);
                    if (context != IntPtr.Zero) wglDeleteContext(context);
                    ReleaseDC(window.Handle, dc);
                }
            }
        }
    }
}
