#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

mod adopt;
const NO_WINDOW: u32 = 0x08000000;
type Result<T> = std::result::Result<T, String>;
trait Explain<T> {
    fn explain(self) -> Result<T>;
}
impl<T, E: std::fmt::Display> Explain<T> for std::result::Result<T, E> {
    fn explain(self) -> Result<T> {
        self.map_err(|e| e.to_string())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    name: String,
    kind: String,
    source: String,
    executable: String,
    color: String,
    number: u32,
    #[serde(default)]
    data_path: Option<String>,
    #[serde(default)]
    home_path: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileView {
    #[serde(flatten)]
    profile: Profile,
    healthy: bool,
    detail: String,
    folder: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    profiles: Vec<ProfileView>,
    existing: Vec<Existing>,
    root: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Existing {
    id: String,
    name: String,
    shortcut: String,
    target: String,
    available: bool,
}
struct Store {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
}
fn profile_dir(store: &Store, id: &str) -> Result<PathBuf> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err("Invalid profile identifier".into());
    }
    Ok(store.root.join("profiles").join(id))
}
fn read_profiles(store: &Store) -> Result<Vec<Profile>> {
    let path = store.root.join("profiles.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    serde_json::from_slice(&fs::read(path).explain()?).explain()
}
fn save_profiles(store: &Store, profiles: &[Profile]) -> Result<()> {
    let dest = store.root.join("profiles.json");
    // Same-volume ReplaceFile preserves the old catalogue if a write is interrupted.
    let temp = store.root.join("profiles.next.json");
    fs::write(&temp, serde_json::to_vec_pretty(profiles).explain()?).explain()?;
    if dest.exists() {
        fs::copy(&dest, store.root.join("profiles.backup.json")).explain()?;
    }
    fs::rename(temp, dest).explain()
}
fn inspect(store: &Store) -> Result<Snapshot> {
    let profiles = read_profiles(store)?
        .into_iter()
        .map(|p| {
            let dir = profile_dir(store, &p.id).unwrap();
            let runtime = dir.join("runtime");
            let healthy = runtime.join(&p.executable).is_file()
                && runtime.join("resources/vendor.asar").is_file()
                && fs::read_to_string(runtime.join("resources/app/index.cjs")).unwrap_or_default()
                    == include_str!("bootstrap.cjs")
                && dir.join("icon.png").is_file();
            ProfileView {
                profile: p,
                healthy,
                detail: if healthy {
                    "فایل‌ها آماده‌اند؛ ورود و اجرای واقعی نیاز به بررسی دارد"
                } else {
                    "فایل اجرایی یا تنظیمات ناقص است؛ ترمیم کنید"
                }
                .into(),
                folder: dir.to_string_lossy().into(),
            }
        })
        .collect();
    Ok(Snapshot {
        profiles,
        existing: read_existing(store)?,
        root: store.root.to_string_lossy().into(),
    })
}
fn read_existing(store: &Store) -> Result<Vec<Existing>> {
    let file = store.root.join("existing.json");
    if !file.exists() {
        return Ok(vec![]);
    }
    serde_json::from_slice(&fs::read(file).explain()?).explain()
}
#[tauri::command]
async fn scan_existing(store: tauri::State<'_, Store>) -> Result<Snapshot> {
    let root = store.root.clone();
    let gate = store.gate.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard=gate.lock().map_err(|_|"Profile store is busy")?;
        let store=Store{root,gate:gate.clone()};
        let script="$ErrorActionPreference='Stop'; $w=New-Object -ComObject WScript.Shell; $rows=@(); $roots=@([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('StartMenu')); foreach($root in $roots){ Get-ChildItem -LiteralPath $root -Filter '*.lnk' -Recurse -ErrorAction SilentlyContinue | Where-Object {$_.BaseName -match '(?i)claude|codex' -and $_.BaseName -notmatch '(?i)helper|startup' -and $_.FullName -notmatch '\\\\Startup\\\\'} | ForEach-Object { $s=$w.CreateShortcut($_.FullName); if($s.Arguments -like '--launch *' -and [IO.Path]::GetFileName($s.TargetPath) -match 'orbit.cloner|orbit-cloner'){return}; $rows+=@{id='';name=$_.BaseName;shortcut=$_.FullName;target=$s.TargetPath;available=[bool](Test-Path -LiteralPath $s.TargetPath)} } }; ConvertTo-Json -InputObject @($rows) -Compress";
        let out=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",script]).creation_flags(NO_WINDOW).output().explain()?;
        if !out.status.success(){return Err(String::from_utf8_lossy(&out.stderr).into());}
        let found:Vec<Existing>=serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim_start_matches('\u{feff}').trim()).explain()?;
        let mut rows=read_existing(&store)?;
        let hidden:Vec<String>=serde_json::from_slice(&fs::read(store.root.join("hidden-existing.json")).unwrap_or_else(|_|b"[]".to_vec())).explain()?;
        for mut row in found {
            if hidden.iter().any(|p|p.eq_ignore_ascii_case(&row.shortcut)||p.eq_ignore_ascii_case(&format!("{}|{}",row.name,row.target))){continue;}
            if let Some(old)=rows.iter_mut().find(|p|p.shortcut.eq_ignore_ascii_case(&row.shortcut)||(p.name==row.name&&p.target==row.target)){row.id=old.id.clone();*old=row;}
            else {row.id=uuid::Uuid::new_v4().to_string();rows.push(row);}
        }
        fs::write(store.root.join("existing.json"),serde_json::to_vec_pretty(&rows).explain()?).explain()?;
        inspect(&store)
    }).await.explain()?
}
#[tauri::command]
fn launch_existing(store: tauri::State<Store>, id: String) -> Result<()> {
    let row = read_existing(&store)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Shortcut not found")?;
    if !Path::new(&row.shortcut).is_file() {
        return Err("Shortcut no longer exists".into());
    }
    Command::new("explorer.exe")
        .arg(row.shortcut)
        .spawn()
        .explain()?;
    Ok(())
}
#[tauri::command]
fn remove_existing(store: tauri::State<Store>, id: String) -> Result<Snapshot> {
    let _guard = store.gate.lock().map_err(|_| "Profile store is busy")?;
    let mut rows = read_existing(&store)?;
    let row = rows
        .iter()
        .find(|p| p.id == id)
        .ok_or("Shortcut not found")?
        .clone();
    let mut hidden: Vec<String> = serde_json::from_slice(
        &fs::read(store.root.join("hidden-existing.json")).unwrap_or_else(|_| b"[]".to_vec()),
    )
    .explain()?;
    hidden.push(format!("{}|{}",row.name,row.target));
    rows.retain(|p| p.id != id);
    fs::write(
        store.root.join("hidden-existing.json"),
        serde_json::to_vec_pretty(&hidden).explain()?,
    )
    .explain()?;
    fs::write(
        store.root.join("existing.json"),
        serde_json::to_vec_pretty(&rows).explain()?,
    )
    .explain()?;
    inspect(&store)
}
#[tauri::command]
fn remove_profile(store: tauri::State<Store>, id: String) -> Result<Snapshot> {
    let _guard = store.gate.lock().map_err(|_| "Profile store is busy")?;
    let mut profiles = read_profiles(&store)?;
    find_profile(&store, &id)?;
    let dir = profile_dir(&store, &id)?;
    let trash = store.root.join("trash");
    fs::create_dir_all(&trash).explain()?;
    let dest = trash.join(format!("{}-{}", id, uuid::Uuid::new_v4()));
    if dir.exists() {
        fs::rename(&dir, &dest).map_err(|e| format!("Close the clone before removing it: {e}"))?;
    }
    profiles.retain(|p| p.id != id);
    if let Err(error) = save_profiles(&store, &profiles) {
        if dest.exists() {
            let _ = fs::rename(&dest, &dir);
        }
        return Err(error);
    }
    let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .delete_subkey_all(format!("Software\\Classes\\Orbit.Profile.{}.Badged", id));
    Ok(inspect(&store)?)
}
#[tauri::command]
fn snapshot(store: tauri::State<Store>) -> Result<Snapshot> {
    inspect(&store)
}
#[tauri::command]
async fn choose_executable() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Windows app", &["exe"])
            .pick_file()
            .map(|p| p.to_string_lossy().into())
    })
    .await
    .ok()
    .flatten()
}
#[derive(Serialize, Deserialize)]
struct Detected {
    name: String,
    path: String,
}
#[tauri::command]
async fn detect_apps() -> Result<Vec<Detected>> {
    tauri::async_runtime::spawn_blocking(|| {
        let script = "$ErrorActionPreference='Stop'; $r=@(); Get-AppxPackage | Where-Object { $_.Name -in @('Claude','OpenAI.Codex') } | ForEach-Object { $n=$_.Name; $base=Join-Path $_.InstallLocation 'app'; foreach($exe in @('claude.exe','ChatGPT.exe','Codex.exe')) { $p=Join-Path $base $exe; if(Test-Path -LiteralPath $p){$r+=@{name=$n;path=$p};break} } }; ConvertTo-Json -InputObject @($r) -Compress";
        let output = Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",script]).creation_flags(NO_WINDOW).output().explain()?;
        if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).into()); }
        serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim_start_matches('\u{feff}').trim()).explain()
    }).await.explain()?
}
fn validate_source(source: &Path, root: &Path) -> Result<PathBuf> {
    let source = fs::canonicalize(source).explain()?;
    if source
        .extension()
        .and_then(|x| x.to_str())
        .map(|s| s.eq_ignore_ascii_case("exe"))
        != Some(true)
    {
        return Err("Select the application's .exe file".into());
    }
    let parent = source.parent().ok_or("Invalid app directory")?;
    let root = fs::canonicalize(root).explain()?;
    if source.starts_with(&root) || root.starts_with(parent) {
        return Err("The original app must be outside the portable data directory".into());
    }
    if !parent.join("resources/app.asar").is_file() {
        return Err(
            "Unsupported application: resources/app.asar was not found next to the executable"
                .into(),
        );
    }
    Ok(source)
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).explain()?;
    for entry in fs::read_dir(from).explain()? {
        let entry = entry.explain()?;
        let ty = entry.file_type().explain()?;
        // Never follow junctions or symlinks outside the selected application.
        use std::os::windows::fs::MetadataExt;
        if entry.metadata().explain()?.file_attributes() & 0x400 != 0 {
            return Err(format!(
                "Reparse point is unsupported: {}",
                entry.path().display()
            ));
        }
        if ty.is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        } else if ty.is_file() {
            fs::copy(entry.path(), to.join(entry.file_name())).explain()?;
        }
    }
    Ok(())
}
const SENTINEL: &[u8] = b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX";
fn patch_fuses(bytes: &mut [u8]) -> Result<()> {
    let matches: Vec<usize> = bytes
        .windows(SENTINEL.len())
        .enumerate()
        .filter(|(_, w)| *w == SENTINEL)
        .map(|(i, _)| i)
        .collect();
    if matches.len() != 1 {
        return Err(
            "Unrecognized Electron executable; no changes were made to the original".into(),
        );
    }
    let start = matches[0] + SENTINEL.len();
    if bytes.get(start) != Some(&1) || bytes.get(start + 1).copied().unwrap_or(0) < 6 {
        return Err("Unsupported Electron fuse version".into());
    }
    for index in [4, 5] {
        let value = bytes
            .get_mut(start + 2 + index)
            .ok_or("Truncated fuse data")?;
        if ![b'0', b'1'].contains(value) {
            return Err("Electron integrity fuse cannot be changed for this app".into());
        }
        *value = b'0';
    }
    Ok(())
}
fn make_icons(dir: &Path, color: &str, number: u32, source: &Path) -> Result<()> {
    let rgb =
        u32::from_str_radix(color.trim_start_matches('#'), 16).map_err(|_| "Invalid color")?;
    let logo=dir.join("original-logo.png");
    let resources=source.parent().ok_or("Invalid app path")?.join("resources");
    let mut original=None;
    // Packaged apps often provide a higher quality logo separately from their launcher.
    for name in ["chatgpt-app-dark.ico","icon-chatgpt.ico","chatgpt-app-light.ico","claude.ico","icon.ico","icon.png"] {
        if let Ok(decoded)=image::open(resources.join(name)) { original=Some(decoded.to_rgba8());break; }
    }
    let original=if let Some(original)=original {original} else {
        let source_text=source.to_string_lossy();
        let shell_source=source_text.strip_prefix(r"\\?\").unwrap_or(&source_text);
        let script=format!("$ErrorActionPreference='Stop';Add-Type -AssemblyName System.Drawing;$i=[System.Drawing.Icon]::ExtractAssociatedIcon({});if(-not $i){{throw 'No associated icon'}};$b=$i.ToBitmap();try{{$b.Save({},[System.Drawing.Imaging.ImageFormat]::Png)}}finally{{$b.Dispose();$i.Dispose()}}",ps_quote(shell_source),ps_quote(&logo.to_string_lossy()));
        let out=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&script]).creation_flags(NO_WINDOW).output().explain()?;
        if !out.status.success(){return Err(format!("استخراج آیکون برنامه ناموفق بود: {}",String::from_utf8_lossy(&out.stderr).trim()));}
        image::open(&logo).explain()?.to_rgba8()
    };
    original.save(&logo).explain()?;
    let mut img=image::imageops::resize(&original,128,128,image::imageops::FilterType::Lanczos3);
    for (x,y,pixel) in img.enumerate_pixels_mut(){
        let dx=x as f32-99.0;let dy=y as f32-99.0;
        if dx*dx+dy*dy<28.0*28.0{*pixel=image::Rgba([(rgb>>16)as u8,(rgb>>8)as u8,rgb as u8,255]);}
    }
    let digits = [
        0b111101101101111u16,
        0b010110010010111,
        0b111001111100111,
        0b111001111001111,
        0b101101111001001,
        0b111100111001111,
        0b111100111101111,
        0b111001001001001,
        0b111101111101111,
        0b111101111001111,
    ];
    let label = number.to_string();
    let scale = if label.len() > 1 { 5 } else { 8 };
    let width = label.len() as u32 * 4 * scale - scale;
    for (i, ch) in label.chars().enumerate() {
        let bits = digits[ch.to_digit(10).unwrap() as usize];
        for y in 0..5 {
            for x in 0..3 {
                if bits & (1 << (14 - y * 3 - x)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            img.put_pixel(
                                (198 - width) / 2 + i as u32 * 4 * scale + x * scale + sx,
                                (198 - 5 * scale) / 2 + y * scale + sy,
                                image::Rgba([255, 255, 255, 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    img.save(dir.join("icon.png")).explain()?;
    img.save(dir.join("icon.ico")).explain()
}
fn build_runtime(store: &Store, p: &Profile) -> Result<()> {
    let source = validate_source(Path::new(&p.source), &store.root)?;
    let dir = profile_dir(store, &p.id)?;
    fs::create_dir_all(&dir).explain()?;
    let staging = dir.join("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).explain()?;
    }
    // Only application files are copied. Profiles, cookies and credentials are never imported.
    copy_tree(source.parent().unwrap(), &staging)?;
    let exe = staging.join(&p.executable);
    let copied_exe=staging.join(source.file_name().unwrap());
    if copied_exe!=exe {fs::rename(&copied_exe,&exe).explain()?;}
    let mut bytes = fs::read(&exe).explain()?;
    // Component Electron distributions keep the fuse wire in chrome.dll.
    let fuse_target = if bytes.windows(SENTINEL.len()).any(|w| w == SENTINEL) {
        exe
    } else {
        let dll = staging.join("chrome.dll");
        bytes = fs::read(&dll).map_err(|_| "Unsupported Electron runtime: fuse wire not found")?;
        dll
    };
    patch_fuses(&mut bytes)?;
    fs::write(fuse_target, bytes).explain()?;
    let resources = staging.join("resources");
    fs::rename(resources.join("app.asar"), resources.join("vendor.asar")).explain()?;
    if resources.join("app.asar.unpacked").exists() {
        fs::rename(
            resources.join("app.asar.unpacked"),
            resources.join("vendor.asar.unpacked"),
        )
        .explain()?;
    }
    let boot = resources.join("app");
    fs::create_dir_all(&boot).explain()?;
    fs::write(
        boot.join("package.json"),
        r#"{"name":"orbit-profile","version":"1.0.0","main":"index.cjs"}"#,
    )
    .explain()?;
    fs::write(boot.join("index.cjs"), include_str!("bootstrap.cjs")).explain()?;
    fs::write(
        boot.join("profile.json"),
        serde_json::to_vec(
            &serde_json::json!({"name":p.name,"dataPath":p.data_path,"preserveName":p.data_path.is_some(),"identity":format!("Orbit.Profile.{}.Badged",p.id)}),
        )
        .explain()?,
    )
    .explain()?;
    make_icons(&dir, &p.color, p.number, &source)?;
    let native_icon_script=dir.join("patch-icon.ps1");
    fs::write(&native_icon_script,include_str!("patch-icon.ps1")).explain()?;
    let output=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(&native_icon_script).arg("-Executable").arg(staging.join(&p.executable)).arg("-Icon").arg(dir.join("icon.ico")).creation_flags(NO_WINDOW).output().explain()?;
    if !output.status.success(){return Err(format!("Copied executable icon update failed: {}",String::from_utf8_lossy(&output.stderr)));}

    let runtime = dir.join("runtime");
    let previous = dir.join("previous-runtime");
    if previous.exists() {
        fs::remove_dir_all(&previous).explain()?;
    }
    if runtime.exists() {
        fs::rename(&runtime, &previous)
            .map_err(|e| format!("Close this clone before repairing it: {e}"))?;
    }
    if let Err(e) = fs::rename(&staging, &runtime) {
        if previous.exists() {
            let _ = fs::rename(previous, runtime);
        }
        return Err(e.to_string());
    }
    Ok(())
}
#[tauri::command]
async fn create_profile(
    store: tauri::State<'_, Store>,
    name: String,
    kind: String,
    source: String,
    color: String,
) -> Result<Snapshot> {
    let root = store.root.clone();
    let gate = store.gate.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.lock().map_err(|_| "Profile store is busy")?;
        let store = Store {
            root,
            gate: gate.clone(),
        };
        if name.trim().is_empty() || name.chars().count() > 48 {
            return Err("Name must contain 1–48 characters".into());
        }
        if !["claude", "codex", "electron"].contains(&kind.as_str()) {
            return Err("Unknown application kind".into());
        }
        if color.len() != 7
            || !color.starts_with('#')
            || !color[1..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Invalid color".into());
        }
        validate_source(Path::new(&source), &store.root)?;
        let mut profiles = read_profiles(&store)?;
        let number = profiles.iter().map(|p| p.number).max().unwrap_or(0) + 1;
        if number > 99 {
            return Err("Up to 99 profiles are supported".into());
        }
        let p = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().into(),
            kind,
            source,
            executable: format!("Orbit-{}.exe",number),
            color,
            number, data_path:None,home_path:None,
        };
        build_runtime(&store, &p)?;
        profiles.push(p);
        save_profiles(&store, &profiles)?;
        inspect(&store)
    })
    .await
    .explain()?
}
#[tauri::command]
async fn repair_profile(
    store: tauri::State<'_, Store>,
    id: String,
    source: Option<String>,
) -> Result<Snapshot> {
    let root = store.root.clone();
    let gate = store.gate.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.lock().map_err(|_| "Profile store is busy")?;
        let store = Store {
            root,
            gate: gate.clone(),
        };
        let mut profiles = read_profiles(&store)?;
        let p = profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Profile not found")?;
        if let Some(source) = source {
            validate_source(Path::new(&source), &store.root)?;
            p.executable = Path::new(&source)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into();
            p.source = source;
        }
        build_runtime(&store, p)?;
        save_profiles(&store, &profiles)?;
        inspect(&store)
    })
    .await
    .explain()?
}
fn find_profile(store: &Store, id: &str) -> Result<Profile> {
    read_profiles(store)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Profile not found".into())
}
fn launch(store: &Store, id: &str, url: Option<String>) -> Result<()> {
    let p = find_profile(store, id)?;
    let dir = profile_dir(store, id)?;
    let exe = dir.join("runtime").join(&p.executable);
    let data = p.data_path.as_ref().map(PathBuf::from).unwrap_or_else(||dir.join("data"));
    let home = p.home_path.as_ref().map(PathBuf::from).unwrap_or_else(||dir.join("home"));
    fs::create_dir_all(&data).explain()?;
    fs::create_dir_all(&home).explain()?;
    let mut cmd = Command::new(exe);
    for (key, _) in std::env::vars_os() {
        let upper = key.to_string_lossy().to_uppercase();
        if [
            "CODEX_",
            "CLAUDE_",
            "ELECTRON_",
            "NODE_",
            "OPENAI_",
            "ANTHROPIC_",
        ]
        .iter()
        .any(|p| upper.starts_with(p))
            || upper == "BUILD_FLAVOR"
        {
            cmd.env_remove(key);
        }
    }
    cmd.current_dir(&dir)
        .arg(format!("--user-data-dir={}", data.display()))
        .env("CODEX_HOME", &home)
        .env("CODEX_ELECTRON_USER_DATA_PATH", &data)
        .env("CLAUDE_CONFIG_DIR", &home);
    if let Some(url) = url {
        if !url.starts_with("claude:") && !url.starts_with("codex:") && !url.starts_with("orbit-") {
            return Err("Unsupported link scheme".into());
        }
        cmd.arg(url);
    }
    cmd.creation_flags(NO_WINDOW).spawn().explain()?;
    let watcher=dir.join("taskbar.ps1");
    fs::write(&watcher,include_str!("taskbar.ps1")).explain()?;
    Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-WindowStyle","Hidden","-File"]).arg(&watcher).arg("-Runtime").arg(dir.join("runtime").join(&p.executable)).arg("-Icon").arg(dir.join("icon.ico")).arg("-Name").arg(&p.name).arg("-Identity").arg(format!("Orbit.Profile.{}.Badged",p.id)).arg("-Manager").arg(std::env::current_exe().explain()?).arg("-Id").arg(&p.id).creation_flags(NO_WINDOW).spawn().explain()?;

    Ok(())
}
#[tauri::command]
fn launch_profile(store: tauri::State<Store>, id: String) -> Result<()> {
    launch(&store, &id, None)
}
#[tauri::command]
fn open_folder(store: tauri::State<Store>, id: Option<String>) -> Result<()> {
    let dir = if let Some(id) = id {
        profile_dir(&store, &id)?
    } else {
        store.root.clone()
    };
    Command::new("explorer.exe").arg(dir).spawn().explain()?;
    Ok(())
}
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
#[tauri::command]
fn create_shortcut(store: tauri::State<Store>, id: String) -> Result<()> {
    let p = find_profile(&store, &id)?;
    let dir = profile_dir(&store, &id)?;
    // The stable UUID filename prevents user labels from becoming filesystem paths.
    let script=format!("$ErrorActionPreference='Stop'; $w=New-Object -ComObject WScript.Shell; $s=$w.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'Orbit-{}.lnk')); $s.TargetPath={}; $s.Arguments={}; $s.WorkingDirectory={}; $s.IconLocation={}; $s.Description={}; $s.Save()",p.id,ps_quote(&std::env::current_exe().explain()?.to_string_lossy()),ps_quote(&format!("--launch {}",id)),ps_quote(&dir.to_string_lossy()),ps_quote(&dir.join("icon.ico").to_string_lossy()),ps_quote(&p.name));
    let result = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(NO_WINDOW)
        .output()
        .explain()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into());
    }
    Ok(())
}
#[tauri::command]
fn register_links(store: tauri::State<Store>, id: String) -> Result<()> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let p = find_profile(&store, &id)?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let progid = format!("Orbit.Profile.{}.Badged", id);
    let (key, _) = hkcu
        .create_subkey(format!("Software\\Classes\\{}", progid))
        .explain()?;
    key.set_value("", &format!("URL:{}", p.name)).explain()?;
    key.set_value("URL Protocol", &"").explain()?;
    let (cmd, _) = key.create_subkey("shell\\open\\command").explain()?;
    cmd.set_value(
        "",
        &format!(
            "\"{}\" --launch {} \"%1\"",
            std::env::current_exe().explain()?.display(),
            id
        ),
    )
    .explain()?;
    let scheme = match p.kind.as_str() {
        "claude" => "claude",
        "codex" => "codex",
        _ => return Err("Link registration supports Claude and Codex only".into()),
    };
    let (open_with, _) = hkcu
        .create_subkey(format!("Software\\Classes\\{}\\OpenWithProgids", scheme))
        .explain()?;
    open_with.set_value(&progid, &"").explain()?;
    Ok(())
}
#[tauri::command]
async fn adopt_claude_family(store:tauri::State<'_,Store>,source:String)->Result<Snapshot>{
 let root=store.root.clone();let gate=store.gate.clone();
 tauri::async_runtime::spawn_blocking(move||{let _lock=gate.lock().map_err(|_|"Store busy")?;adopt::adopt_family(&Store{root,gate:gate.clone()},source)}).await.explain()?
}
fn main() {
    let root = std::env::current_exe()
        .expect("Executable path")
        .parent()
        .unwrap()
        .join("portable-data");
    if let Err(error) = fs::create_dir_all(&root) {
        rfd::MessageDialog::new()
            .set_title("Orbit Cloner")
            .set_description(format!("Place Orbit Cloner in a writable folder. {error}"))
            .show();
        return;
    }
    let store = Store {
        root,
        gate: Arc::new(Mutex::new(())),
    };
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--launch") {
        if let Some(id) = args.get(2) {
            if let Err(error) = launch(&store, id, args.get(3).cloned()) {
                rfd::MessageDialog::new()
                    .set_title("Orbit Cloner")
                    .set_description(error)
                    .show();
            }
        }
        return;
    }
    if args.get(1).map(String::as_str)==Some("--repair-claude-family") {
        let result=args.get(2).ok_or("Missing source executable".to_string()).and_then(|source|adopt::adopt_family(&store,source.clone()));
        let report=match result {Ok(_)=>serde_json::json!({"ok":true}),Err(e)=>serde_json::json!({"ok":false,"error":e})};
        let _=fs::write(store.root.join("repair-result.json"),serde_json::to_vec(&report).unwrap());return;
    }
    if args.get(1).map(String::as_str)==Some("--refresh-icons") {
        if let Some(id)=args.get(2){let result=find_profile(&store,id).and_then(|p|{let dir=profile_dir(&store,id)?;make_icons(&dir,&p.color,p.number,Path::new(&p.source))?;fs::write(dir.join("runtime/resources/app/index.cjs"),include_str!("bootstrap.cjs")).explain()});let _=fs::write(store.root.join("icon-result.json"),serde_json::to_vec(&serde_json::json!({"ok":result.is_ok(),"error":result.err()})).unwrap());}return;
    }
    tauri::Builder::default()
        .manage(store)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            adopt_claude_family,
            scan_existing,
            launch_existing,
            remove_existing,
            remove_profile,
            choose_executable,
            detect_apps,
            create_profile,
            repair_profile,
            launch_profile,
            open_folder,
            create_shortcut,
            register_links
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Orbit Cloner");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changes_only_integrity_fuses() {
        let mut bytes = [SENTINEL, &[1, 8], b"11001101", b"tail"].concat();
        let before = bytes.clone();
        patch_fuses(&mut bytes).unwrap();
        let base = SENTINEL.len() + 2;
        for i in 0..bytes.len() {
            if i == base + 4 || i == base + 5 {
                assert_eq!(bytes[i], b'0');
            } else {
                assert_eq!(bytes[i], before[i]);
            }
        }
    }
    #[test]
    fn rejects_unknown_or_ambiguous_runtime() {
        assert!(patch_fuses(&mut [0; 80]).is_err());
        let mut bytes = [SENTINEL, SENTINEL].concat();
        assert!(patch_fuses(&mut bytes).is_err());
    }
    #[test]
    fn rejects_path_traversal() {
        let s = Store {
            root: PathBuf::from("data"),
            gate: Arc::new(Mutex::new(())),
        };
        assert!(profile_dir(&s, "../../private").is_err());
    }
    #[test]
    #[ignore = "Requires an installed Electron application; writes only to target/orbit-smoke"]
    fn installed_runtime_build_and_repair() {
        let source = std::env::var("ORBIT_TEST_SOURCE")
            .expect("Set ORBIT_TEST_SOURCE to the original Electron exe");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/orbit-smoke");
        fs::create_dir_all(&root).unwrap();
        let store = Store {
            root,
            gate: Arc::new(Mutex::new(())),
        };
        let p = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Orbit smoke test".into(),
            kind: "claude".into(),
            executable: Path::new(&source)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into(),
            source,
            color: "#698dcc".into(),
            number: 1,data_path:None,home_path:None,
        };
        build_runtime(&store, &p).unwrap();
        let dir = profile_dir(&store, &p.id).unwrap();
        fs::create_dir_all(dir.join("data")).unwrap();
        fs::write(dir.join("data/preservation-test.txt"), "keep this profile").unwrap();
        build_runtime(&store, &p).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("data/preservation-test.txt")).unwrap(),
            "keep this profile"
        );
        save_profiles(&store, &[p]).unwrap();
        save_profiles(&store, &read_profiles(&store).unwrap()).unwrap();
        assert!(inspect(&store).unwrap().profiles[0].healthy);
        println!("Smoke test root: {}", store.root.display());
    }
}
