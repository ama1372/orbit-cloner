param([string]$Runtime,[string]$Icon,[string]$Name,[string]$Identity,[string]$Manager,[string]$Id)
$ErrorActionPreference='Stop'
$mutex=New-Object Threading.Mutex($false,('Local\OrbitTaskbar'+$Id))
if(-not $mutex.WaitOne(0)){exit}
try {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class OrbitTaskbar {
 [StructLayout(LayoutKind.Sequential)] public struct Key { public Guid format; public uint id; public Key(uint n){format=new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");id=n;} }
 [StructLayout(LayoutKind.Explicit,Size=24)] public struct Value { [FieldOffset(0)] public ushort type; [FieldOffset(8)] public IntPtr text; }
 [ComImport,Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)] public interface Store {
  void GetCount(out uint count);void GetAt(uint i,out Key key);void GetValue(ref Key key,out Value value);void SetValue(ref Key key,ref Value value);void Commit();
 }
 [DllImport("shell32.dll")] static extern int SHGetPropertyStoreForWindow(IntPtr window,ref Guid iid,out Store store);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadImage(IntPtr module,string file,uint type,int width,int height,uint flags);
 [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr hwnd,uint msg,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool DestroyIcon(IntPtr icon);
 [DllImport("ole32.dll")] static extern int PropVariantClear(ref Value value);
 public static bool Verify(IntPtr hwnd,string identity,IntPtr big){var guid=typeof(Store).GUID;Store store;if(SHGetPropertyStoreForWindow(hwnd,ref guid,out store)!=0)return false;try{var key=new Key(5);Value value;store.GetValue(ref key,out value);try{return Marshal.PtrToStringUni(value.text)==identity && SendMessage(hwnd,0x7f,new IntPtr(1),IntPtr.Zero)==big;}finally{PropVariantClear(ref value);}}finally{Marshal.ReleaseComObject(store);}}
 static void Set(Store store,uint id,string text){var key=new Key(id);var value=new Value{type=31,text=Marshal.StringToCoTaskMemUni(text)};try{store.SetValue(ref key,ref value);}finally{Marshal.FreeCoTaskMem(value.text);}}
 public static void Apply(IntPtr hwnd,string identity,string name,string icon,string command,IntPtr big,IntPtr small){
  var guid=typeof(Store).GUID;Store store;if(SHGetPropertyStoreForWindow(hwnd,ref guid,out store)!=0)return;
  try{Set(store,5,identity);Set(store,2,command);Set(store,3,icon+",0");Set(store,4,name);store.Commit();}finally{Marshal.ReleaseComObject(store);}
  SendMessage(hwnd,0x80,new IntPtr(1),big);SendMessage(hwnd,0x80,IntPtr.Zero,small);
 }
}
'@
$big=[OrbitTaskbar]::LoadImage([IntPtr]::Zero,$Icon,1,32,32,16)
$small=[OrbitTaskbar]::LoadImage([IntPtr]::Zero,$Icon,1,16,16,16)
$empty=0
do {
 $processes=@(Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Runtime)) -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $Runtime})
 if($processes.Count){$empty=0}else{$empty++}
 foreach($p in $processes){if($p.MainWindowHandle -ne [IntPtr]::Zero){[OrbitTaskbar]::Apply($p.MainWindowHandle,$Identity,$Name,$Icon,('"'+$Manager+'" --launch '+$Id),$big,$small);@{verified=[OrbitTaskbar]::Verify($p.MainWindowHandle,$Identity,$big)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path (Split-Path $Icon) 'taskbar-status.json')}}
 Start-Sleep -Seconds 2
}while($empty -lt 15)
} finally {
 if($big){[void][OrbitTaskbar]::DestroyIcon($big)}
 if($small){[void][OrbitTaskbar]::DestroyIcon($small)}
 $mutex.ReleaseMutex();$mutex.Dispose()
}
