using Microsoft.Win32;
using System.Diagnostics;
using System.IO;
using WeComLauncher.Native;

namespace WeComLauncher.Services;

/// <summary>
/// Resolves the official WXWork.exe without shipping or patching any binary.
/// </summary>
public sealed class WeComPathResolver
{
    private static readonly string[] CommonPaths =
    [
        @"C:\Program Files (x86)\WXWork\WXWork.exe",
        @"C:\Program Files\WXWork\WXWork.exe",
        @"D:\Program Files (x86)\WXWork\WXWork.exe",
        @"D:\Program Files\WXWork\WXWork.exe",
    ];

    private string? _cached;

    public string? Resolve(string? overridePath = null)
    {
        if (!string.IsNullOrWhiteSpace(overridePath) && File.Exists(overridePath))
        {
            _cached = overridePath;
            return _cached;
        }

        if (!string.IsNullOrWhiteSpace(_cached) && File.Exists(_cached))
            return _cached;

        var fromRegistry = TryRegistry();
        if (fromRegistry is not null)
            return _cached = fromRegistry;

        var fromProcess = TryRunningProcess();
        if (fromProcess is not null)
            return _cached = fromProcess;

        foreach (var path in CommonPaths)
        {
            if (File.Exists(path))
                return _cached = path;
        }

        return null;
    }

    private static string? TryRegistry()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Tencent\WXWork");
            var install = key?.GetValue("Executable") as string
                          ?? key?.GetValue("InstallPath") as string;
            if (string.IsNullOrWhiteSpace(install))
                return null;

            if (install.EndsWith(".exe", StringComparison.OrdinalIgnoreCase) && File.Exists(install))
                return install;

            var candidate = Path.Combine(install.TrimEnd('\\'), "WXWork.exe");
            return File.Exists(candidate) ? candidate : null;
        }
        catch
        {
            return null;
        }
    }

    private static string? TryRunningProcess()
    {
        foreach (var proc in Process.GetProcessesByName("WXWork"))
        {
            try
            {
                var path = QueryImagePath(proc.Id);
                if (!string.IsNullOrWhiteSpace(path) && File.Exists(path))
                    return path;
            }
            catch
            {
                // ignore access-denied processes
            }
            finally
            {
                proc.Dispose();
            }
        }

        return null;
    }

    private static string? QueryImagePath(int pid)
    {
        var handle = NativeMethods.OpenProcess(
            NativeMethods.PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (handle == IntPtr.Zero)
            return null;

        try
        {
            var sb = new System.Text.StringBuilder(1024);
            var size = sb.Capacity;
            if (!NativeMethods.QueryFullProcessImageName(handle, 0, sb, ref size))
                return null;
            return sb.ToString();
        }
        finally
        {
            NativeMethods.CloseHandle(handle);
        }
    }
}
