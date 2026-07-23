using System.Diagnostics;
using System.Runtime.InteropServices;
using WeComLauncher.Native;

namespace WeComLauncher.Services;

/// <summary>
/// Releases WeCom single-instance named mutex handles from *outside* the process.
///
/// Low-signature rules enforced here:
/// - No CreateRemoteThread / LoadLibrary injection
/// - No WriteProcessMemory into WXWork
/// - No renaming / replacing WXWork.exe
/// - Only DuplicateHandle(..., DUPLICATE_CLOSE_SOURCE) on matching mutex names
/// </summary>
public sealed class MutexReleaseService
{
    /// <summary>
    /// Known ExclusiveObject name prefixes used by recent WeCom builds.
    /// Keep this list data-driven; update when WeCom renames the mutex.
    /// </summary>
    private static readonly string[] MutexNameHints =
    [
        "Tencent.WeWork.ExclusiveObject",
        "Tencent.WeWork.ExclusiveObjectInstance",
        "Tencent.WeWork.Exclusive",
    ];

    public MutexReleaseResult ReleaseWeComMutexes()
    {
        var pids = FindWeComPids();
        if (pids.Count == 0)
            return MutexReleaseResult.Ok("未发现运行中的企业微信，可直接启动。");

        var closed = 0;
        var errors = new List<string>();

        foreach (var pid in pids)
        {
            try
            {
                closed += CloseMatchingMutexHandles(pid);
            }
            catch (UnauthorizedAccessException)
            {
                errors.Add($"PID {pid}: 权限不足，请以管理员身份重试。");
            }
            catch (Exception ex)
            {
                errors.Add($"PID {pid}: {ex.Message}");
            }
        }

        if (closed == 0 && errors.Count > 0)
            return MutexReleaseResult.Fail(string.Join(" ", errors));

        return MutexReleaseResult.Ok(
            closed > 0
                ? $"已释放 {closed} 个互斥句柄。"
                : "未匹配到互斥句柄（可能已被释放或版本更名）。");
    }

    private static List<int> FindWeComPids()
    {
        var list = new List<int>();
        foreach (var p in Process.GetProcessesByName("WXWork"))
        {
            list.Add(p.Id);
            p.Dispose();
        }
        return list;
    }

    private static int CloseMatchingMutexHandles(int pid)
    {
        // Enumerate system handles then filter by owning PID + object name.
        // This is the same class of technique used by Sysinternals Handle / Process Explorer.
        var handles = EnumerateProcessHandles(pid);
        var closed = 0;

        var process = NativeMethods.OpenProcess(
            NativeMethods.PROCESS_DUP_HANDLE | NativeMethods.PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid);
        if (process == IntPtr.Zero)
            throw new UnauthorizedAccessException();

        try
        {
            foreach (var h in handles)
            {
                if (!TryGetObjectName(process, h, out var name) || string.IsNullOrEmpty(name))
                    continue;

                if (!MutexNameHints.Any(hint =>
                        name.Contains(hint, StringComparison.OrdinalIgnoreCase)))
                    continue;

                if (NativeMethods.DuplicateHandle(
                        process,
                        h,
                        NativeMethods.GetCurrentProcess(),
                        out var localDup,
                        0,
                        false,
                        NativeMethods.DUPLICATE_CLOSE_SOURCE))
                {
                    if (localDup != IntPtr.Zero)
                        NativeMethods.CloseHandle(localDup);
                    closed++;
                }
            }
        }
        finally
        {
            NativeMethods.CloseHandle(process);
        }

        return closed;
    }

    private static List<IntPtr> EnumerateProcessHandles(int pid)
    {
        // Prefer SystemExtendedHandleInformation (64). Fall back gracefully.
        var bufferSize = 1024 * 1024;
        var result = new List<IntPtr>();

        for (var attempt = 0; attempt < 4; attempt++)
        {
            var buffer = Marshal.AllocHGlobal(bufferSize);
            try
            {
                var status = NativeMethods.NtQuerySystemInformation(
                    NativeMethods.SystemExtendedHandleInformation,
                    buffer,
                    bufferSize,
                    out var returnLength);

                if ((uint)status == NativeMethods.STATUS_INFO_LENGTH_MISMATCH)
                {
                    bufferSize = Math.Max(bufferSize * 2, returnLength + 65536);
                    continue;
                }

                if (status != 0)
                    return result;

                // SYSTEM_HANDLE_INFORMATION_EX: ULONG_PTR NumberOfHandles; ULONG_PTR Reserved; HANDLE_TABLE_ENTRY[...]
                var numberOfHandles = Marshal.ReadIntPtr(buffer).ToInt64();
                var entrySize = Marshal.SizeOf<NativeMethods.SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX>();
                var offset = IntPtr.Size * 2;

                for (long i = 0; i < numberOfHandles; i++)
                {
                    var entryPtr = IntPtr.Add(buffer, offset + (int)(i * entrySize));
                    var entry = Marshal.PtrToStructure<NativeMethods.SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX>(entryPtr);
                    if (entry.UniqueProcessId.ToInt32() == pid)
                        result.Add(entry.HandleValue);
                }

                return result;
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        return result;
    }

    private static bool TryGetObjectName(IntPtr process, IntPtr remoteHandle, out string name)
    {
        name = string.Empty;

        if (!NativeMethods.DuplicateHandle(
                process,
                remoteHandle,
                NativeMethods.GetCurrentProcess(),
                out var local,
                0,
                false,
                NativeMethods.DUPLICATE_SAME_ACCESS))
        {
            return false;
        }

        try
        {
            // NtQueryObject(ObjectNameInformation) — kept in a tiny helper to avoid injecting anything.
            return NtObjectName.TryQuery(local, out name);
        }
        finally
        {
            NativeMethods.CloseHandle(local);
        }
    }
}

/// <summary>Thin wrapper around NtQueryObject ObjectNameInformation.</summary>
internal static class NtObjectName
{
    private const int ObjectNameInformation = 1;

    [DllImport("ntdll.dll")]
    private static extern int NtQueryObject(
        IntPtr handle,
        int objectInformationClass,
        IntPtr objectInformation,
        int objectInformationLength,
        out int returnLength);

    [StructLayout(LayoutKind.Sequential)]
    private struct UNICODE_STRING
    {
        public ushort Length;
        public ushort MaximumLength;
        public IntPtr Buffer;
    }

    public static bool TryQuery(IntPtr handle, out string name)
    {
        name = string.Empty;
        var length = 1024;
        var buffer = Marshal.AllocHGlobal(length);
        try
        {
            var status = NtQueryObject(handle, ObjectNameInformation, buffer, length, out var needed);
            if (status == unchecked((int)0xC0000004) /* STATUS_INFO_LENGTH_MISMATCH */)
            {
                Marshal.FreeHGlobal(buffer);
                length = needed + 64;
                buffer = Marshal.AllocHGlobal(length);
                status = NtQueryObject(handle, ObjectNameInformation, buffer, length, out _);
            }

            if (status != 0)
                return false;

            var us = Marshal.PtrToStructure<UNICODE_STRING>(buffer);
            if (us.Length == 0 || us.Buffer == IntPtr.Zero)
                return false;

            name = Marshal.PtrToStringUni(us.Buffer, us.Length / 2) ?? string.Empty;
            return name.Length > 0;
        }
        catch
        {
            return false;
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }
}

public readonly record struct MutexReleaseResult(bool Success, string Message)
{
    public static MutexReleaseResult Ok(string msg) => new(true, msg);
    public static MutexReleaseResult Fail(string msg) => new(false, msg);
}
