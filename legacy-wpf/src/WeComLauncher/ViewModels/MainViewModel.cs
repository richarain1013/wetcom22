using System.Collections.ObjectModel;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using WeComLauncher.Models;
using WeComLauncher.Services;

namespace WeComLauncher.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private readonly InstanceManager _manager = new();
    private readonly SettingsStore _store = new();
    private AppSettings _settings;
    private CancellationTokenSource? _batchCts;

    public ObservableCollection<AccountSlot> Slots { get; } = new();
    public ObservableCollection<string> Logs { get; } = new();

    [ObservableProperty] private string? _exePath;
    [ObservableProperty] private int _batchCount = 8;
    [ObservableProperty] private int _minDelayMs = 2500;
    [ObservableProperty] private int _maxDelayMs = 6000;
    [ObservableProperty] private bool _preferRegistry = true;
    [ObservableProperty] private bool _isBusy;
    [ObservableProperty] private string _statusText = "就绪";
    [ObservableProperty] private int _runningCount;

    public MainViewModel()
    {
        _settings = _store.Load();
        ExePath = _settings.CustomExePath ?? _manager.ResolveExe(null);
        BatchCount = _settings.DefaultBatchCount;
        MinDelayMs = _settings.MinDelayMs;
        MaxDelayMs = _settings.MaxDelayMs;
        PreferRegistry = _settings.PreferRegistryFlag;

        for (var i = 0; i < AppSettings.MaxSlots; i++)
        {
            var alias = i < _settings.SlotAliases.Count
                ? _settings.SlotAliases[i]
                : $"账号 {i + 1}";
            Slots.Add(new AccountSlot { Index = i + 1, Alias = alias });
        }

        _manager.Log += msg => Application.Current.Dispatcher.Invoke(() =>
        {
            Logs.Insert(0, msg);
            if (Logs.Count > 200)
                Logs.RemoveAt(Logs.Count - 1);
        });

        RefreshRunning();
    }

    [RelayCommand]
    private void BrowseExe()
    {
        var dlg = new Microsoft.Win32.OpenFileDialog
        {
            Filter = "企业微信|WXWork.exe|可执行文件|*.exe",
            Title = "选择 WXWork.exe",
        };
        if (dlg.ShowDialog() == true)
        {
            ExePath = dlg.FileName;
            Persist();
        }
    }

    [RelayCommand]
    private void DetectExe()
    {
        ExePath = _manager.ResolveExe(null);
        StatusText = string.IsNullOrEmpty(ExePath) ? "未检测到企业微信" : $"已定位: {ExePath}";
        Persist();
    }

    [RelayCommand]
    private async Task LaunchOneAsync()
    {
        if (IsBusy) return;
        var slot = Slots.FirstOrDefault(s => s.Status != InstanceStatus.Running)
                   ?? Slots.FirstOrDefault();
        if (slot is null) return;

        IsBusy = true;
        slot.Status = InstanceStatus.Starting;
        slot.LastError = null;
        try
        {
            var result = await _manager.LaunchOneAsync(BuildOptions(1));
            if (result.Success)
            {
                slot.Status = InstanceStatus.Running;
                slot.Pid = result.Pid;
                slot.StartedAt = DateTimeOffset.Now;
                StatusText = result.Message;
            }
            else
            {
                slot.Status = InstanceStatus.Error;
                slot.LastError = result.Message;
                StatusText = result.Message;
            }
        }
        finally
        {
            IsBusy = false;
            RefreshRunning();
        }
    }

    [RelayCommand]
    private async Task LaunchBatchAsync()
    {
        if (IsBusy) return;
        IsBusy = true;
        _batchCts = new CancellationTokenSource();
        StatusText = $"正在分批启动 {BatchCount} 个实例…";

        foreach (var s in Slots.Take(BatchCount))
        {
            s.Status = InstanceStatus.Idle;
            s.Pid = null;
            s.LastError = null;
        }

        try
        {
            var progress = new Progress<int>(n =>
            {
                StatusText = $"已启动 {n}/{BatchCount}";
                if (n <= Math.Min(BatchCount, Slots.Count))
                {
                    var slot = Slots[n - 1];
                    slot.Status = InstanceStatus.Running;
                    slot.StartedAt = DateTimeOffset.Now;
                }
            });

            var results = await _manager.LaunchBatchAsync(
                BuildOptions(BatchCount),
                progress,
                _batchCts.Token);

            for (var i = 0; i < results.Count && i < Slots.Count; i++)
            {
                var r = results[i];
                var slot = Slots[i];
                if (r.Success)
                {
                    slot.Status = InstanceStatus.Running;
                    slot.Pid = r.Pid;
                    slot.StartedAt ??= DateTimeOffset.Now;
                }
                else
                {
                    slot.Status = InstanceStatus.Error;
                    slot.LastError = r.Message;
                }
            }

            StatusText = $"批量完成：成功 {results.Count(r => r.Success)} / 尝试 {results.Count}";
        }
        catch (OperationCanceledException)
        {
            StatusText = "已取消批量启动";
        }
        finally
        {
            IsBusy = false;
            Persist();
            RefreshRunning();
        }
    }

    [RelayCommand]
    private void CancelBatch()
    {
        _batchCts?.Cancel();
    }

    [RelayCommand]
    private void KillAll()
    {
        _manager.KillAll();
        foreach (var s in Slots)
        {
            s.Status = InstanceStatus.Idle;
            s.Pid = null;
        }
        RefreshRunning();
        StatusText = "已关闭全部实例";
    }

    [RelayCommand]
    private void RefreshRunning()
    {
        var pids = _manager.ListRunningPids();
        RunningCount = pids.Count;
        StatusText = $"运行中企业微信进程: {RunningCount}";
    }

    [RelayCommand]
    private void Persist()
    {
        _settings.CustomExePath = ExePath;
        _settings.DefaultBatchCount = BatchCount;
        _settings.MinDelayMs = MinDelayMs;
        _settings.MaxDelayMs = MaxDelayMs;
        _settings.PreferRegistryFlag = PreferRegistry;
        _settings.SlotAliases = Slots.Select(s => s.Alias).ToList();
        _store.Save(_settings);
        StatusText = "设置已保存";
    }

    private LaunchOptions BuildOptions(int count) => new()
    {
        Count = count,
        ExePath = ExePath,
        MinDelayMs = MinDelayMs,
        MaxDelayMs = MaxDelayMs,
        PreferRegistryFlag = PreferRegistry,
    };
}
