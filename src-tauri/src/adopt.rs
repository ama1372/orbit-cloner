use super::*;
pub fn adopt_family(store: &Store, source: String) -> Result<Snapshot> {
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA missing")?).join("Anthropic/ClaudeFamily");
    // MSIX redirects the apparent LOCALAPPDATA directory into LocalCache.
    let virtual_base=PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()).join("Packages/Claude_pzs8sxrjxfjjc/LocalCache/Local/Anthropic/ClaudeFamily");
    let actual_base=if base.join("app-data/Local State").is_file(){base.clone()}else{virtual_base};
    let data = actual_base.join("app-data");
    let home = if actual_base.join("claude-code").is_dir(){actual_base.join("claude-code")}else{base.join("claude-code")};
    if !data.join("Local State").is_file() { return Err("Recognized ClaudeFamily account not found; nothing changed".into()); }
    let check = format!("$p=@(Get-CimInstance Win32_Process -Filter \"name='claude.exe'\" | Where-Object {{$_.CommandLine -like '*{}*' -or $_.CommandLine -like '*{}*'}});if($p.Count){{exit 20}}", data.display(),base.display());
    if !Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&check]).creation_flags(NO_WINDOW).status().explain()?.success() { return Err("ابتدا Claude 2 را کامل ببندید؛ حساب پاک نمی‌شود.".into()); }
    let mut profiles = read_profiles(store)?;
    if profiles.iter().any(|p|p.data_path.as_deref()==Some(data.to_string_lossy().as_ref())) { return Err("این حساب قبلاً منتقل شده؛ از ترمیم نسخه در فهرست Orbit استفاده کنید.".into()); }
    validate_source(Path::new(&source), &store.root)?;
    let executable = "Claude-2-Orbit.exe".into();
    let p = Profile { id:uuid::Uuid::new_v4().to_string(), name:"Claude 2".into(),kind:"claude".into(),source,executable,color:"#698dcc".into(),number:2,data_path:Some(data.to_string_lossy().into()),home_path:Some(home.to_string_lossy().into()) };
    build_runtime(store,&p)?;
    let dir = profile_dir(store,&p.id)?;
    copy_tree(&data,&dir.join("account-backup/app-data"))?;
    if home.exists() { copy_tree(&home,&dir.join("account-backup/claude-code"))?; }
    fs::write(dir.join("adoption.json"),serde_json::to_vec_pretty(&p).explain()?).explain()?;
    profiles.push(p.clone());save_profiles(store,&profiles)?;
    let backups=dir.join("shortcut-backups");fs::create_dir_all(&backups).explain()?;
    let manager=std::env::current_exe().explain()?;
    let script=format!("$ErrorActionPreference='Stop';$w=New-Object -ComObject WScript.Shell;foreach($scope in @('Desktop','Programs')){{$folder=[Environment]::GetFolderPath($scope);$file=Join-Path $folder 'Claude 2.lnk';if(Test-Path -LiteralPath $file){{Copy-Item -LiteralPath $file -Destination (Join-Path {} ($scope+'.lnk'))}};$s=$w.CreateShortcut($file);$s.TargetPath={};$s.Arguments={};$s.IconLocation={};$s.Save()}}",ps_quote(&backups.to_string_lossy()),ps_quote(&manager.to_string_lossy()),ps_quote(&format!("--launch {}",p.id)),ps_quote(&dir.join("icon.ico").to_string_lossy()));
    let script=script+&format!(";$pin=Join-Path $env:APPDATA 'Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar\\Claude - Family.lnk';if(Test-Path -LiteralPath $pin){{Copy-Item -LiteralPath $pin -Destination (Join-Path {} 'Pinned.lnk');$s=$w.CreateShortcut($pin);$s.TargetPath={};$s.Arguments={};$s.IconLocation={};$s.Save()}}",ps_quote(&backups.to_string_lossy()),ps_quote(&manager.to_string_lossy()),ps_quote(&format!("--launch {}",p.id)),ps_quote(&dir.join("icon.ico").to_string_lossy()));
    let out=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&script]).creation_flags(NO_WINDOW).output().explain()?;
    if !out.status.success(){return Err("Account backed up; shortcut update failed. Launch from Orbit.".into());}
    let mut existing=read_existing(store)?;existing.retain(|row|!row.name.eq_ignore_ascii_case("Claude 2"));
    fs::write(store.root.join("existing.json"),serde_json::to_vec_pretty(&existing).explain()?).explain()?;
    inspect(store)
}
