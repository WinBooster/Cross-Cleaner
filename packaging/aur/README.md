# Cross Cleaner — AUR

Файлы для публикации Cross Cleaner в Arch User Repository. Это отдельный
git-репозиторий на AUR, в основной репозиторий проекта он попадать не должен.

## Состав

| Файл | Назначение |
|---|---|
| `PKGBUILD` | Скрипт сборки пакета |
| `.SRCINFO` | Метаданные для веб-интерфейса AUR |
| `LICENSE` | 0BSD — лицензия на файлы пакета (требование Arch) |
| `.gitignore` | Исключает артефакты `makepkg` |
| `README.md` | Этот файл (для людей, а не для AUR) |

## Важно: веб-формы отправки больше нет

Кнопка «Submit Package» на aur.archlinux.org **отсутствует** — AUR перешёл на
отправку через git по SSH. Репозиторий пакета создаётся первым же `git push` на
пустой репозиторий, отдельная регистрация не нужна.

## Публикация

### 1. Аккаунт AUR

Зарегистрируйтесь на https://aur.archlinux.org/passwd/ и войдите. Пакет
`cross-cleaner` на момент написания свободен (проверено через
`https://aur.archlinux.org/rpc/v5/info` — 0 результатов).

### 2. SSH-ключ

Для записи нужен отдельный SSH-ключ — **не переиспользуйте ваш основной
GitHub-ключ**, его можно будет отозвать отдельно:

```bash
ssh-keygen -f ~/.ssh/aur
```

Публичный ключ (`~/.ssh/aur.pub`) вставьте в профиль на AUR: «My Account» →
«Add SSH Key». И добавьте в `~/.ssh/config`:

```
Host aur.archlinux.org
  IdentityFile ~/.ssh/aur
  User aur
```

Отпечатки сервера AUR (с главной страницы) для проверки:

```
Ed25519  SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4
ECDSA    SHA256:uTa/0PndEgPZTf76e1DFqXKJEXKsn7m9ivhLQtzGOCI
RSA      SHA256:5s5cIyReIfNNVGRFdDbe3hdYiI5OelHGpw2rOUud3Q8
```

### 3. Клонировать пустой репозиторий

AUR клонирует пустой репозиторий и сам создаёт pkgbase:

```bash
cd packaging/aur
git -c init.defaultBranch=master clone ssh://aur@aur.archlinux.org/cross-cleaner.git /tmp/aur-cross-cleaner
cd /tmp/aur-cross-cleaner
cp /home/roman/Documents/GitHub/Cross-Cleaner/packaging/aur/{PKGBUILD,.SRCINFO,LICENSE,.gitignore} .
```

Предупреждение «You appear to have cloned an empty repository» — это ожидаемо,
не ошибка.

### 4. Пуш

```bash
makepkg --printsrcinfo -p PKGBUILD > .SRCINFO   # всегда перед коммитом
git add PKGBUILD .SRCINFO LICENSE .gitignore
git commit -m "Initial AUR package"
git push
```

После пуша пакет появится на AUR в течение от нескольких минут до часа.

**Требования AUR, которые ломают пуш:**

- Пуш только в ветку `master`. Если ветка называется иначе — переименуйте:
  `git branch -M master`.
- `PKGBUILD` и `.SRCINFO` должны быть в коммите. Если забыли `.SRCINFO` —
  `git commit --amend --add .SRCINFO`, а не новый коммит.
- `LICENSE` обязателен: пакеты без лицензии не промотируются в официальные
  репозитории.
- Имя пользователя и email коммитов берутся из глобального git-конфига, и после
  пуша их сменить почти невозможно. Если для AUR нужны другие — задайте
  локально до коммита:
  `git config user.name "..." && git config user.email "..."`.

### 5. Обновление пакета

```bash
cd /tmp/aur-cross-cleaner
git pull
# ...внести правки...
makepkg --printsrcinfo -p PKGBUILD > .SRCINFO
git commit -am "описание изменения"
git push
```

После установки:

```bash
paru -S cross-cleaner
```

## Проверка перед отправкой

```bash
bash -n PKGBUILD                      # синтаксис
makepkg --printsrcinfo -p PKGBUILD   # метаданные
makepkg -s --noconfirm               # полная сборка с установкой зависимостей
makepkg --packagelist                # какие файлы попадут в пакет
```

Полная сборка долгая: ~636 крейтов в lock-файле плюс `lto = true`,
`codegen-units = 1`, `opt-level = "z"` из workspace-профиля. Это настройка
upstream, а не этого PKGBUILD.

## Особенности пакета

**Собирается из git, а не из tarball.** `source` указывает на репозиторий,
версия вычисляется через `git describe`. Новый тег upstream подхватывается сам:
orphanage увидит изменившуюся версию, руками править ничего не нужно.

По правилам AUR, VCS-пакет, не привязанный к конкретной версии, должен
называться с суффиксом `-git`. Здесь `pkgname=cross-cleaner` без суффикса —
это отступление от рекомендации: пакет отслеживает теги (`git describe`
даёт версию релиза, а не `0.0.rN.gHASH`), то есть фактически привязан к
конкретным версиям. Если хотите строго по правилам — переименуйте в
`cross-cleaner-git`, но тогда и в `url`/`.desktop`/README ничего менять не
нужно, только `pkgname` в PKGBUILD.

**Три бинарника.** `cross-cleaner` (оконное приложение), `cross-cleaner-tui`
(терминальное) и `cross-cleaner-cli` (для скриптов). `cargo` называет их по
именам пакетов (`desktop`, `tui`, `cli`), в PKGBUILD они переименовываются,
чтобы `Exec=` в desktop-файлах не зависел от конкретного релиза.

**`--no-default-features` везде.** Путь самообновления (проверка GitHub на новый
релиз и замена собственного исполняемого файла) выключен намеренно: версией
установленного пакета управляет пакетный менеджер, и приложение не должно ему
мешать. Так же поступает `.deb`-сборка — см. `packaging/linux/nfpm.yaml`.

**Зависимости.** Список `depends` перенесён из `nfpm.yaml`, где он тоже
поддерживается вручную: winit через `x11-dl` открывает весь X11-стек через
`dlopen`, поэтому эти библиотеки не видны ни в `ldd`, ни в проверках
зависимостей, но без них окно не откроется.

**Иконка.** `crates/winicon/assets/icon.ico` содержит один кадр 96×96, и
`icotool` извлекает его как есть. В отличие от AppImage-сборки, здесь ничего не
растягивается под 128 и 256 — установлен единственный настоящий размер. Чтобы
добавить остальные, нужен мастер-PNG ≥512px в `crates/winicon/assets`.

**Метаданные AppStream.** `packaging/linux/cross-cleaner.appdata.xml` содержит
плейсхолдеры `%%VERSION%%` и `%%DATE%%`, которые в релизе подставляет workflow.
Здесь то же самое делает `prepare()` — по вычисленному `pkgver` и дате коммита.

**Две лицензии.** `LICENSE` в этом каталоге (0BSD) — лицензия на файлы пакета,
требование Arch для допуска в официальные репозитории. Лицензия самой программы
(GPL-3.0-or-later) указана в поле `license=` PKGBUILD и в `LICENSE` основного
репозитория проекта.

## Что осталось от служебных слов в репозитории проекта

Заменены на `Cross-Cleaner`: ссылки на GitHub в README/BUILD/CONTRIBUTING/
build_setup.iss, homepage на `https://cross-cleaner.github.io/`, maintainer и
vendor в `nfpm.yaml`, developer в appdata.

Оставлены как есть, потому что это идентификаторы во внешних реестрах — их
смена означает новый пакет/приложение, а не правку ссылок:

- `.github/workflows/publish.yml:49,65` — winget IDs `WinBooster.Cross_Cleaner_GUI`
  и `WinBooster.Cross_Cleaner_TUI`. Смена потребует нового пакета в
  `microsoft/winget-pkgs`.
- `crates/android/Cargo.toml:43` и `crates/android/src/lib.rs:49` —
  `com.winbooster.crosscleaner`. Это application ID: смена создаст новое
  приложение в Google Play, а не обновление существующего.