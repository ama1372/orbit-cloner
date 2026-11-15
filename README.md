# Orbit Cloner

A portable Windows profile manager built with **Rust + Tauri 2 + Svelte 5**. Persian, right-to-left interface. MIT licensed.

> **0.1 preview:** intended for experimentation with locally installed Electron applications. Account isolation, concurrent operation, vendor updates, notifications and browser authentication callbacks require application-specific testing. This is not a Windows security sandbox.

## فارسی

Orbit برای ساخت فضای مستقل از برنامه‌های Electron مثل Claude و Codex است. برنامهٔ اصلی انتخاب می‌شود و یک کپی با پوشهٔ داده، رنگ، شماره و هویت تسک‌بار جدا ساخته می‌شود.

- شناسایی نصب‌های Store کلاد و کدکس و انتخاب دستی فایل EXE
- ساخت کپی مستقل؛ بدون واردکردن حساب‌ها یا داده‌های کلون‌های قدیمی
- پوشهٔ مستقل برای داده‌های Electron و تنظیمات CLI
- رنگ و شماره برای آیکون پنجره و Tray در برنامه‌های سازگار
- ساخت میان‌بر دسکتاپ و ثبت گزینهٔ بازکردن لینک، بدون تغییر برنامهٔ پیش‌فرض
- ترمیم **پروفایل‌های ساخته‌شده در Orbit** با حفظ پوشهٔ داده و نگه‌داشتن نسخهٔ قبلی فایل‌های اجرایی
- تغییر منبع برای وقتی برنامهٔ اصلی به‌روزرسانی شده است
- اسکن خودکار میان‌برهای Claude و Codex در دسکتاپ و Start؛ اجرای نسخه‌های قبلی و حذف از فهرست
- حذف نسخه‌های Orbit با تأیید و انتقال فایل‌ها به پوشهٔ محلی `trash`

برای نسخه‌های قدیمی فقط اجرا و حذف از فهرست در دسترس است. ترمیم مستقیم کلون‌های ساخته‌شده با ابزارهای دیگر، جداسازی قطعی OAuth و انتقال حساب‌های قبلی در این نسخه پیاده نشده است.

## Run the portable app

Download the Windows artifact from this repository's Actions runs, or build locally. Extract the ZIP to a writable folder and run `Orbit Cloner.exe`. No installer or administrator access is needed. Windows x64 and the Microsoft Edge WebView2 runtime are required; WebView2 is normally present on Windows 10/11.

All manager data lives in `portable-data/` beside the EXE. Keep that directory with the executable when moving it. Application runtimes can take several gigabytes per profile. Profiles containing vendor credentials may remain bound to the original Windows account through DPAPI; portability does **not** guarantee credential portability.

1. Choose **پروفایل جدید**, then select the original app EXE. It must have `resources/app.asar` beside it.
2. Choose the profile name and color. Read the experimental-copy disclosure and create it. The executable path is under Advanced settings.
3. Launch the clone and sign in manually. Verify both original and clone before relying on them.
4. Close the clone before using Repair. If the original was updated or moved, choose Change source.

Desktop shortcuts and registry link entries contain absolute paths. Recreate them after moving Orbit. Link registration adds an `OpenWithProgids` entry for `claude:` or `codex:`; Windows/browser policy decides whether a chooser appears. It does not rewrite OAuth redirects or take over the default handler.

## How isolation works and its limits

Only installed application files are copied. An early bootstrap sets independent Electron `userData` / `sessionData`, app name and AppUserModelID, and wraps CommonJS BrowserWindow/Tray constructors for profile icons. CLI homes are set through the vendor environment variables. Original application files and existing user profiles are not modified.

The copied Electron runtime's ASAR integrity and ASAR-only fuses are disabled so it can load the bootstrap before the vendor archive. **This invalidates the copied binary's signature and weakens integrity checks in that copy.** Other fuses, including cookie encryption, are preserved. See the official [Electron fuses documentation](https://www.electronjs.org/docs/latest/tutorial/fuses) and [Electron app API](https://www.electronjs.org/docs/latest/api/app).

Copies are unsupported vendor modifications. Applications can override isolation, use machine-wide credential stores, rely on package identity, use fixed OAuth callbacks, or stop working after an update. Native modules and ESM imports can bypass some wrappers, particularly tray icon replacement. Auto-updating copies through Electron's updater is disabled; external vendor update mechanisms may differ. Use Change source / Repair to refresh copies manually.

Repair checks bootstrap and essential file presence. A green status means files are present, not that authentication or complete isolation has been verified. Failures during vendor startup may appear in the profile's `runtime.log`. A failed initial build may leave an unlisted staging directory; it contains application files only.

The startup scan reads only matching shortcut metadata; it does not inspect old accounts or copy old data. Removing an external entry hides it in Orbit without deleting its shortcut or application. Removing an Orbit profile moves its directory into `portable-data/trash/`; this retains its disk usage and credentials until you manually remove that directory. Close the clone first. Old desktop shortcuts may need to be removed manually.

Local verification: the installed-runtime test passed against Claude, including repair and data preservation. Its vendor main module loaded and displayed its login/network screen in an isolated copy. Sign-in was not completed; the connection was blocked by network/Cloudflare conditions. Codex account isolation, browser callbacks, and tray behavior remain unverified.

## Build and verify

Prerequisites: Windows x64, Node.js 22+, Rust stable with the MSVC toolchain, Visual Studio C++ Build Tools, WebView2.

```powershell
npm ci
npm run check
npm run tauri build -- --no-bundle
cargo test --manifest-path src-tauri/Cargo.toml --release
```

The executable is `src-tauri/target/release/orbit-cloner.exe`. Copy it to a writable folder and rename it if desired. No Node.js or Rust installation is required on the destination machine.

An optional installed-runtime test copies an application into the ignored build directory, rebuilds it, and checks data preservation and catalogue replacement. It does not sign in or touch existing profiles:

```powershell
$env:ORBIT_TEST_SOURCE = 'C:\path\to\app.exe'
cargo test --manifest-path src-tauri/Cargo.toml --release installed_runtime_build_and_repair -- --ignored --nocapture
```

Do not commit `portable-data/`, runtime copies, credentials, screenshots containing private data, or vendor executables. This repository distributes only Orbit's own code; vendor software remains subject to its original license. Orbit is an independent project, unaffiliated with OpenAI or Anthropic.
