# isku-probe.ps1 - READ ONLY look at a Roccat keyboard over HID.
# Only touches devices with vendor ID 0x1E7D (Roccat).
# It only calls HidD_GetFeature. There is no HidD_SetFeature, WriteFile or
# firmware call in this file, so it cannot change anything on the keyboard.
# Result is saved next to this script as isku-probe-result.txt.

$source = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

public static class IskuProbe
{
    const uint DIGCF_PRESENT = 0x02, DIGCF_DEVICEINTERFACE = 0x10;
    const uint GENERIC_READ = 0x80000000, GENERIC_WRITE = 0x40000000;
    const uint FILE_SHARE_RW = 0x03, OPEN_EXISTING = 3;
    const int HIDP_STATUS_SUCCESS = 0x00110000;
    const ushort ROCCAT_VID = 0x1E7D;

    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVICE_INTERFACE_DATA
    {
        public int cbSize;
        public Guid InterfaceClassGuid;
        public int Flags;
        public IntPtr Reserved;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct HIDD_ATTRIBUTES
    {
        public int Size;
        public ushort VendorID;
        public ushort ProductID;
        public ushort VersionNumber;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct HIDP_CAPS
    {
        public ushort Usage;
        public ushort UsagePage;
        public ushort InputReportByteLength;
        public ushort OutputReportByteLength;
        public ushort FeatureReportByteLength;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 17)]
        public ushort[] Reserved;
        public ushort NumberLinkCollectionNodes;
        public ushort NumberInputButtonCaps;
        public ushort NumberInputValueCaps;
        public ushort NumberInputDataIndices;
        public ushort NumberOutputButtonCaps;
        public ushort NumberOutputValueCaps;
        public ushort NumberOutputDataIndices;
        public ushort NumberFeatureButtonCaps;
        public ushort NumberFeatureValueCaps;
        public ushort NumberFeatureDataIndices;
    }

    [DllImport("hid.dll")]
    static extern void HidD_GetHidGuid(out Guid guid);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_GetAttributes(SafeFileHandle h, ref HIDD_ATTRIBUTES a);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_GetPreparsedData(SafeFileHandle h, out IntPtr data);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_FreePreparsedData(IntPtr data);

    [DllImport("hid.dll")]
    static extern int HidP_GetCaps(IntPtr data, ref HIDP_CAPS caps);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_GetFeature(SafeFileHandle h, byte[] buffer, int length);

    [DllImport("hid.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool HidD_GetProductString(SafeFileHandle h, byte[] buffer, int length);

    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr SetupDiGetClassDevsW(ref Guid g, IntPtr e, IntPtr p, uint flags);

    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr set, IntPtr info, ref Guid g,
        uint index, ref SP_DEVICE_INTERFACE_DATA data);

    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetupDiGetDeviceInterfaceDetailW(IntPtr set,
        ref SP_DEVICE_INTERFACE_DATA data, IntPtr detail, int size,
        out int required, IntPtr info);

    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFileW(string name, uint access, uint share,
        IntPtr security, uint disposition, uint flags, IntPtr template);

    static List<string> GetHidPaths()
    {
        List<string> paths = new List<string>();
        Guid guid;
        HidD_GetHidGuid(out guid);
        IntPtr set = SetupDiGetClassDevsW(ref guid, IntPtr.Zero, IntPtr.Zero,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if (set == new IntPtr(-1)) return paths;
        try
        {
            for (uint i = 0; ; i++)
            {
                SP_DEVICE_INTERFACE_DATA data = new SP_DEVICE_INTERFACE_DATA();
                data.cbSize = Marshal.SizeOf(typeof(SP_DEVICE_INTERFACE_DATA));
                if (!SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref guid, i, ref data)) break;
                int required;
                SetupDiGetDeviceInterfaceDetailW(set, ref data, IntPtr.Zero, 0, out required, IntPtr.Zero);
                IntPtr detail = Marshal.AllocHGlobal(required);
                try
                {
                    Marshal.WriteInt32(detail, IntPtr.Size == 8 ? 8 : 6);
                    if (SetupDiGetDeviceInterfaceDetailW(set, ref data, detail, required, out required, IntPtr.Zero))
                        paths.Add(Marshal.PtrToStringUni(new IntPtr(detail.ToInt64() + 4)));
                }
                finally { Marshal.FreeHGlobal(detail); }
            }
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
        return paths;
    }

    static string Hex(byte[] b, int n)
    {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < n && i < b.Length; i++)
        {
            if (i > 0) s.Append(i % 16 == 0 ? "\r\n        " : " ");
            s.Append(b[i].ToString("x2"));
        }
        return s.ToString();
    }

    static SafeFileHandle Open(string path, out string mode)
    {
        SafeFileHandle h = CreateFileW(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_RW,
            IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        mode = "read/write handle";
        if (!h.IsInvalid) return h;
        h.Dispose();
        h = CreateFileW(path, GENERIC_READ, FILE_SHARE_RW, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        mode = "read handle";
        if (!h.IsInvalid) return h;
        h.Dispose();
        h = CreateFileW(path, 0, FILE_SHARE_RW, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        mode = "query-only handle";
        return h;
    }

    public static string Run()
    {
        // id, expected size, name. Sizes come from the Linux driver's hid-roccat-isku.h.
        object[][] reports = new object[][] {
            new object[] { 0x04, 0x03, "control (status)" },
            new object[] { 0x05, 0x03, "actual_profile" },
            new object[] { 0x07, 0x06, "key_mask" },
            new object[] { 0x08, 0x29, "keys_function" },
            new object[] { 0x09, 0x41, "keys_easyzone" },
            new object[] { 0x0a, 0x1d, "keys_media" },
            new object[] { 0x0b, 0x17, "keys_thumbster" },
            new object[] { 0x0d, 0x23, "keys_macro" },
            new object[] { 0x0f, 0x06, "info" },
            new object[] { 0x10, 0x10, "light" },
            new object[] { 0x13, 0x06, "keys_capslock" },
            new object[] { 0x14, 0x14, "last_set" }
        };

        StringBuilder o = new StringBuilder();
        List<string> paths = GetHidPaths();
        o.AppendLine("HID interfaces on this PC: " + paths.Count);
        int found = 0;

        foreach (string path in paths)
        {
            string mode;
            using (SafeFileHandle h = Open(path, out mode))
            {
                if (h.IsInvalid) continue;
                HIDD_ATTRIBUTES a = new HIDD_ATTRIBUTES();
                a.Size = Marshal.SizeOf(typeof(HIDD_ATTRIBUTES));
                if (!HidD_GetAttributes(h, ref a) || a.VendorID != ROCCAT_VID) continue;
                found++;

                HIDP_CAPS caps = new HIDP_CAPS();
                IntPtr pre;
                if (HidD_GetPreparsedData(h, out pre))
                {
                    int status = HidP_GetCaps(pre, ref caps);
                    HidD_FreePreparsedData(pre);
                    if (status != HIDP_STATUS_SUCCESS) o.AppendLine("HidP_GetCaps failed");
                }

                byte[] name = new byte[256];
                string product = HidD_GetProductString(h, name, name.Length)
                    ? Encoding.Unicode.GetString(name).TrimEnd('\0') : "(no name)";

                o.AppendLine();
                o.AppendLine("=== " + product + "  VID_" + a.VendorID.ToString("X4")
                    + " PID_" + a.ProductID.ToString("X4")
                    + " firmware field " + a.VersionNumber.ToString("X4"));
                o.AppendLine("usage page " + caps.UsagePage.ToString("X4")
                    + " usage " + caps.Usage.ToString("X4")
                    + " | input " + caps.InputReportByteLength
                    + " output " + caps.OutputReportByteLength
                    + " feature " + caps.FeatureReportByteLength
                    + " | " + mode);
                o.AppendLine(path);

                if (caps.FeatureReportByteLength == 0)
                {
                    o.AppendLine("no feature reports on this interface");
                    continue;
                }

                foreach (object[] r in reports)
                {
                    int id = (int)r[0], size = (int)r[1];
                    string label = "0x" + id.ToString("x2") + " " + (string)r[2];
                    byte[] buffer = new byte[caps.FeatureReportByteLength];
                    buffer[0] = (byte)id;
                    if (HidD_GetFeature(h, buffer, buffer.Length))
                    {
                        o.AppendLine(label);
                        o.AppendLine("        " + Hex(buffer, Math.Min(size, buffer.Length)));
                        continue;
                    }
                    int error = Marshal.GetLastWin32Error();
                    if (size != buffer.Length)
                    {
                        byte[] exact = new byte[size];
                        exact[0] = (byte)id;
                        if (HidD_GetFeature(h, exact, exact.Length))
                        {
                            o.AppendLine(label + " (exact size)");
                            o.AppendLine("        " + Hex(exact, exact.Length));
                            continue;
                        }
                        error = Marshal.GetLastWin32Error();
                    }
                    o.AppendLine(label + " FAILED, Windows error " + error);
                }
            }
        }

        o.AppendLine();
        o.AppendLine("Roccat interfaces found: " + found);
        return o.ToString();
    }
}
'@

$resultFile = Join-Path $PSScriptRoot 'isku-probe-result.txt'
try {
    Add-Type -TypeDefinition $source -Language CSharp
    $result = [IskuProbe]::Run()
} catch {
    $result = "Probe failed: $($_ | Out-String)"
}
[IO.File]::WriteAllText($resultFile, $result)
Write-Host $result
Write-Host "Saved to $resultFile"
