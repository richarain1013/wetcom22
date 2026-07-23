using Microsoft.Win32;

namespace WeComLauncher.Services;

/// <summary>
/// Soft probe for the undocumented HKCU multi_instances flag.
/// If present and accepted by the installed build, no mutex close is needed.
/// Never patches binaries; only writes a user-scope DWORD.
/// </summary>
public sealed class RegistryMultiInstanceService
{
    private const string KeyPath = @"Software\Tencent\WXWork";
    private const string ValueName = "multi_instances";

    public bool TryEnable(int maxInstances = 10)
    {
        try
        {
            using var key = Registry.CurrentUser.CreateSubKey(KeyPath);
            if (key is null)
                return false;

            var current = key.GetValue(ValueName);
            if (current is int i && i >= maxInstances)
                return true;

            key.SetValue(ValueName, maxInstances, RegistryValueKind.DWord);
            return true;
        }
        catch
        {
            return false;
        }
    }

    public int? ReadConfiguredMax()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(KeyPath);
            return key?.GetValue(ValueName) as int?;
        }
        catch
        {
            return null;
        }
    }
}
