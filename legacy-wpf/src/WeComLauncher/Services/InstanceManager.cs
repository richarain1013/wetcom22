using System.Diagnostics;
using System.IO;
using System.Text.Json;
using WeComLauncher.Models;

namespace WeComLauncher.Services;

public sealed class SettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
    };

    private readonly string _path;

    public SettingsStore()
    {
        var dir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "WeComLauncher");
        Directory.CreateDirectory(dir);
        _path = Path.Combine(dir, "settings.json");
    }

    public AppSettings Load()
    {
        try
        {
            if (!File.Exists(_path))
                return new AppSettings();

            var json = File.ReadAllText(_path);
            return JsonSerializer.Deserialize<AppSettings>(json, JsonOptions) ?? new AppSettings();
        }
        catch
        {
            return new AppSettings();
        }
    }

    public void Save(AppSettings settings)
    {
        var json = JsonSerializer.Serialize(settings, JsonOptions);
        File.WriteAllText(_path, json);
    }
}

/// <summary>
/// Orchestrates path resolve → (optional registry) → mutex release → paced CreateProcess.
/// Does not touch WeCom memory or modules.
/// </summary>
public sealed class InstanceManager
{
    private readonly WeComPathResolver _paths = new();
    private readonly RegistryMultiInstanceService _registry = new();
    private readonly MutexReleaseService _mutex = new();
    private readonly LaunchPolicy _policy = new();
    private readonly object _gate = new();

    public event Action<string>? Log;

    public LaunchPolicy Policy => _policy;

    public string? ResolveExe(string? overridePath) => _paths.Resolve(overridePath);

    public async Task<LaunchResult> LaunchOneAsync(
        LaunchOptions options,
        CancellationToken ct = default)
    {
        await _policy.WaitBeforeNextLaunchAsync(ct);

        var exe = _paths.Resolve(options.ExePath);
        if (exe is null)
            return new LaunchResult { Success = false, Message = "未找到 WXWork.exe，请手动指定路径。" };

        if (options.PreferRegistryFlag)
            _registry.TryEnable(AppSettings.MaxSlots);

        // Always attempt mutex release when an instance may already be running.
        // Harmless if none exist; required for 2nd–Nth instance on most builds.
        var release = _mutex.ReleaseWeComMutexes();
        Emit(release.Message);
        if (!release.Success)
            return new LaunchResult { Success = false, Message = release.Message };

        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = exe,
                WorkingDirectory = Path.GetDirectoryName(exe) ?? string.Empty,
                UseShellExecute = true, // normal shell launch — looks like a user double-click
            };

            Process? proc;
            lock (_gate)
            {
                proc = Process.Start(psi);
            }

            if (proc is null)
                return new LaunchResult { Success = false, Message = "CreateProcess 失败。" };

            Emit($"已启动实例 PID={proc.Id}");
            return new LaunchResult
            {
                Success = true,
                Pid = proc.Id,
                Message = $"启动成功 PID={proc.Id}",
            };
        }
        catch (Exception ex)
        {
            return new LaunchResult { Success = false, Message = ex.Message };
        }
    }

    public async Task<IReadOnlyList<LaunchResult>> LaunchBatchAsync(
        LaunchOptions options,
        IProgress<int>? progress = null,
        CancellationToken ct = default)
    {
        var count = LaunchPolicy.ClampCount(options.Count);
        _policy.MinDelayMs = options.MinDelayMs;
        _policy.MaxDelayMs = options.MaxDelayMs;
        _policy.Reset();

        var results = new List<LaunchResult>(count);
        for (var i = 0; i < count; i++)
        {
            ct.ThrowIfCancellationRequested();
            var one = await LaunchOneAsync(options, ct);
            results.Add(one);
            progress?.Report(i + 1);

            if (!one.Success)
            {
                Emit($"批量启动在第 {i + 1} 个实例处中止: {one.Message}");
                break;
            }
        }

        return results;
    }

    public IReadOnlyList<int> ListRunningPids()
    {
        return Process.GetProcessesByName("WXWork")
            .Select(p =>
            {
                var id = p.Id;
                p.Dispose();
                return id;
            })
            .ToList();
    }

    public int KillAll()
    {
        var n = 0;
        foreach (var p in Process.GetProcessesByName("WXWork"))
        {
            try
            {
                p.Kill(entireProcessTree: true);
                n++;
            }
            catch
            {
                // ignore
            }
            finally
            {
                p.Dispose();
            }
        }

        Emit($"已结束 {n} 个企业微信进程。");
        return n;
    }

    private void Emit(string msg) => Log?.Invoke($"[{DateTime.Now:HH:mm:ss}] {msg}");
}
