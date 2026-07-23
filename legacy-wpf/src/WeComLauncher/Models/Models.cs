using CommunityToolkit.Mvvm.ComponentModel;

namespace WeComLauncher.Models;

public enum InstanceStatus
{
    Idle,
    Starting,
    Running,
    Stopping,
    Error
}

public partial class AccountSlot : ObservableObject
{
    public int Index { get; init; }

    [ObservableProperty] private string _alias = string.Empty;
    [ObservableProperty] private InstanceStatus _status = InstanceStatus.Idle;
    [ObservableProperty] private int? _pid;
    [ObservableProperty] private string? _lastError;
    [ObservableProperty] private DateTimeOffset? _startedAt;
}

public sealed class LaunchOptions
{
    /// <summary>Target concurrent accounts (8–10 typical).</summary>
    public int Count { get; set; } = 1;

    /// <summary>Min delay between launches to avoid burst fingerprints.</summary>
    public int MinDelayMs { get; set; } = 2500;

    /// <summary>Max delay between launches (exclusive upper jitter bound).</summary>
    public int MaxDelayMs { get; set; } = 6000;

    /// <summary>Override WXWork.exe path; null = auto-detect.</summary>
    public string? ExePath { get; set; }

    /// <summary>
    /// Prefer registry multi_instances probe before mutex release.
    /// Falls back to mutex release if registry path is unavailable.
    /// </summary>
    public bool PreferRegistryFlag { get; set; } = true;
}

public sealed class AppSettings
{
    public const int MaxSlots = 10;

    public string? CustomExePath { get; set; }
    public int DefaultBatchCount { get; set; } = 8;
    public int MinDelayMs { get; set; } = 2500;
    public int MaxDelayMs { get; set; } = 6000;
    public bool PreferRegistryFlag { get; set; } = true;
    public List<string> SlotAliases { get; set; } = Enumerable.Range(1, MaxSlots)
        .Select(i => $"账号 {i}")
        .ToList();
}

public sealed class LaunchResult
{
    public bool Success { get; init; }
    public int? Pid { get; init; }
    public string Message { get; init; } = string.Empty;
}
