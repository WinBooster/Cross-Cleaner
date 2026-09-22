# Cross Cleaner Updater

Авто-обновлятор для Windows. Качает `Cross_Cleaner_Setup.exe` (Inno Setup) из GitHub Releases репозитория `WinBooster/Cross-Cleaner` и запускает его в тихом режиме.

Бинарь собирается вместе с основным проектом и ставится рядом с `Cross_Cleaner_GUI.exe` в `{app}` (Program Files/Cross Cleaner).

## Как работает

1. Читает текущую версию:
   - `--current-version` (приоритет)
   - реестр `HKLM/HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{63C69122-AC80-4866-B328-5C3188A01F76}_is1` → `DisplayVersion` (пишет Inno Setup)
   - `APP_VERSION` (env при сборке релиза)
   - `CARGO_PKG_VERSION` (fallback)
2. Запрашивает `https://api.github.com/repos/<repo>/releases/latest`, ищет asset `Cross_Cleaner_Setup.exe`
3. Сравнивает версии (`parse_version` / `is_newer` — как в `database::version`, поддерживает `v2.0.2.8.2` с произвольным числом сегментов)
4. Скачивает в `%TEMP%\Cross_Cleaner_Setup_<version>.exe` (копия как `Cross_Cleaner_Setup.exe`)
5. Запускает Inno Setup с флагами:
   - `/SP- /SUPPRESSMSGBOXES /CLOSEAPPLICATIONS /RESTARTAPPLICATIONS`
   - `/SILENT` (default) или `/VERYSILENT` или без ключа (интерактив)
   - доп. флаги через `--installer-args`
   - UAC запрашивает сам установщик (Inno manifest)

## Сборка

```bash
cargo build -p updater --release
# или весь workspace
cargo build --release
```

Бинарь: `target/release/updater.exe`

Добавлен в `build_setup.iss`:
```iss
Source: "target\release\updater.exe"; DestDir: "{app}"; Flags: ignoreversion
```
Workflow `release.yml` уже собирает весь workspace, поэтому `updater.exe` попадает в `Cross_Cleaner_Setup.exe` автоматически.

## Использование

```bash
# проверить
updater --check
updater --check --json --current-version 2.0.2.8.2

# интерактивно (спросит Y/n)
updater

# тихо скачать и поставить
updater --mode silent --yes

# совсем без UI
updater --mode very-silent --yes

# только скачать
updater --download-only --out-dir C:\Temp

# форсить переустановку
updater --force --yes

# кастомный репозиторий / ассет
updater --repo WinBooster/Cross-Cleaner --asset Cross_Cleaner_Setup.exe

# передать доп. ключи Inno Setup
updater --installer-args "/LOG=C:\Temp\cc_update.log"

# JSON для GUI
updater --check --json
# {"current_version":"2.0.3.7.1","latest_version":"2.0.3.7.1","update_available":false,...}
# exit code  0 = update available, 2 = already latest (в --check режиме)
```

### Интеграция с GUI

GUI уже проверяет `database::version::check_new_version()` и показывает баннер. Для авто-обновления:

```rust
// в egui / winit потоке
let updater = std::path::Path::new(&install_dir).join("updater.exe");
if updater.exists() {
    std::process::Command::new(updater)
        .args(["--mode", "silent", "--yes"])
        .spawn()
        .ok();
} else {
    // fallback: скачать updater или открыть страницу релиза
    open::that(database::version::RELEASES_URL).ok();
}
```

Или через `--json`:
```bash
updater --check --json | jq .update_available
```

### Inno Setup флаги (справка)

| Флаг | Описание |
|------|----------|
| `/SILENT` | прогресс виден, подтверждения нет |
| `/VERYSILENT` | вообще без окон |
| `/SUPPRESSMSGBOXES` | давит MessageBox |
| `/CLOSEAPPLICATIONS` | закрывает приложения с `closeapplications` |
| `/SP-` | отключает "This will install..." |

Workflow релиза (`release.yml`) автоматически обновляет `build_setup.iss: #define MyAppVersion` и компилирует инсталлер через `Minionguyjpro/Inno-Setup-Action`.

## Только Windows

На Linux/macOS `updater` поддерживает только `--check`/`--json` (проверку). Установка через Inno Setup — только Windows.
