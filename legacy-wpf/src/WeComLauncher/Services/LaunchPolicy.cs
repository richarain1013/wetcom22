namespace WeComLauncher.Services;

/// <summary>
/// Launch pacing for 8–10 accounts.
/// Goal: look like sequential human launches, not a burst bot.
/// </summary>
public sealed class LaunchPolicy
{
    private readonly Random _rng = new();
    private DateTimeOffset _lastLaunchAt = DateTimeOffset.MinValue;

    public int MinDelayMs { get; set; } = 2500;
    public int MaxDelayMs { get; set; } = 6000;

    /// <summary>
    /// Wait until the next launch window. Adds jitter between Min/Max.
    /// First call in a session returns immediately.
    /// </summary>
    public async Task WaitBeforeNextLaunchAsync(CancellationToken ct = default)
    {
        if (_lastLaunchAt == DateTimeOffset.MinValue)
        {
            _lastLaunchAt = DateTimeOffset.UtcNow;
            return;
        }

        var elapsed = (int)(DateTimeOffset.UtcNow - _lastLaunchAt).TotalMilliseconds;
        var target = _rng.Next(Math.Min(MinDelayMs, MaxDelayMs), Math.Max(MinDelayMs, MaxDelayMs) + 1);
        var remain = target - elapsed;
        if (remain > 0)
            await Task.Delay(remain, ct);

        _lastLaunchAt = DateTimeOffset.UtcNow;
    }

    public void Reset() => _lastLaunchAt = DateTimeOffset.MinValue;

    /// <summary>
    /// Soft ceiling — product supports up to 10; discourage larger bursts.
    /// </summary>
    public static int ClampCount(int requested) => Math.Clamp(requested, 1, Models.AppSettings.MaxSlots);
}
