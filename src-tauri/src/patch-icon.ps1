param([string]$Executable,[string]$Icon)
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class OrbitIconResource {
 delegate bool Names(IntPtr m,IntPtr type,IntPtr name,IntPtr p);
 delegate bool Languages(IntPtr m,IntPtr type,IntPtr name,ushort lang,IntPtr p);
 class Resource {public string name;public ushort number;public ushort language;}
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr LoadLibraryEx(string file,IntPtr reserved,uint flags);
 [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern bool EnumResourceNames(IntPtr m,IntPtr type,Names callback,IntPtr p);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern bool EnumResourceLanguages(IntPtr m,IntPtr type,IntPtr name,Languages callback,IntPtr p);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr BeginUpdateResource(string file,bool delete);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool UpdateResource(IntPtr h,IntPtr type,IntPtr name,ushort language,byte[] data,uint size);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool EndUpdateResource(IntPtr h,bool discard);
 public static void Patch(string exe,string ico){
  byte[] bytes=File.ReadAllBytes(ico);if(BitConverter.ToUInt16(bytes,4)!=1)throw new Exception("Expected a single-image ICO");
  int size=(int)BitConverter.ToUInt32(bytes,14),offset=(int)BitConverter.ToUInt32(bytes,18);
  byte[] image=new byte[size];Array.Copy(bytes,offset,image,0,size);
  byte[] group=new byte[20];Array.Copy(bytes,0,group,0,6);Array.Copy(bytes,6,group,6,12);group[18]=0x60;group[19]=0xEA; // 60000
  var resources=new List<Resource>();var module=LoadLibraryEx(exe,IntPtr.Zero,2);if(module==IntPtr.Zero)throw new Win32Exception();
  try {EnumResourceNames(module,new IntPtr(14),delegate(IntPtr m,IntPtr t,IntPtr n,IntPtr p){
    string text=n.ToInt64()>65535?Marshal.PtrToStringUni(n):null;ushort number=text==null?(ushort)n.ToInt64():(ushort)0;
    EnumResourceLanguages(m,t,n,delegate(IntPtr a,IntPtr b,IntPtr c,ushort lang,IntPtr d){resources.Add(new Resource{name=text,number=number,language=lang});return true;},IntPtr.Zero);return true;
  },IntPtr.Zero);}finally{FreeLibrary(module);}
  if(resources.Count==0)throw new Exception("Executable has no icon group");
  var handle=BeginUpdateResource(exe,false);if(handle==IntPtr.Zero)throw new Win32Exception();bool commit=false;
  try{foreach(var resource in resources){IntPtr name=resource.name==null?new IntPtr(resource.number):Marshal.StringToHGlobalUni(resource.name);try{
   if(!UpdateResource(handle,new IntPtr(3),new IntPtr(60000),resource.language,image,(uint)image.Length)||!UpdateResource(handle,new IntPtr(14),name,resource.language,group,(uint)group.Length))throw new Win32Exception();
  }finally{if(resource.name!=null)Marshal.FreeHGlobal(name);}}commit=true;}finally{if(!EndUpdateResource(handle,!commit)&&commit)throw new Win32Exception();}
 }
}
'@
[OrbitIconResource]::Patch($Executable,$Icon)
