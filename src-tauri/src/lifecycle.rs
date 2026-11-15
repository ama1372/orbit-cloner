use super::*;

// Deletion is explicitly confirmed by the UI. Close only executables inside
// this managed runtime, including its bundled app server, never installed apps.
pub fn close_runtime(dir: &Path) -> Result<()> {
    let runtime=dir.join("runtime");
    let script=format!("$ErrorActionPreference='Stop';$root={};$items=@(Get-Process -ErrorAction SilentlyContinue | Where-Object {{$_.Path -and $_.Path.StartsWith($root,[StringComparison]::OrdinalIgnoreCase)}});foreach($p in $items){{try{{if($p.MainWindowHandle -ne [IntPtr]::Zero){{[void]$p.CloseMainWindow()}}}}catch{{}}}};Start-Sleep -Milliseconds 900;Get-Process -ErrorAction SilentlyContinue | Where-Object {{$_.Path -and $_.Path.StartsWith($root,[StringComparison]::OrdinalIgnoreCase)}} | Stop-Process -Force -ErrorAction SilentlyContinue;$helper={};Get-CimInstance Win32_Process -Filter \"name='powershell.exe'\" | Where-Object {{$_.ProcessId -ne $PID -and $_.CommandLine -and $_.CommandLine.Contains($helper)}} | ForEach-Object {{Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}}",ps_quote(&(runtime.to_string_lossy().to_string()+"\\")),ps_quote(&dir.join("taskbar.ps1").to_string_lossy()));
    let out=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&script]).creation_flags(NO_WINDOW).output().explain()?;
    if !out.status.success(){return Err("بستن این نسخه ممکن نشد؛ برنامهٔ اصلی و حساب‌ها تغییری نکردند.".into());}
    Ok(())
}

pub fn move_to_trash(from: &Path, to: &Path) -> Result<()> {
    for attempt in 0..40 {
        match fs::rename(from,to) {
            Ok(())=>return Ok(()),
            Err(error) if attempt<39 && matches!(error.raw_os_error(),Some(5|32|33))=> {
                // Windows can retain image and crash-handler locks briefly
                // after Stop-Process has returned. Never change account ACLs.
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            Err(error)=>return Err(format!("ویندوز هنوز فایل‌های این نسخه را قفل کرده است؛ داده‌ها حفظ شدند: {error}")),
        }
    }
    unreachable!()
}
