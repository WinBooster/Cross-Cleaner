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

Итоговый `PKGBUILD` в AUR генерируется из этого каталога воркфлоу
`.github/workflows/aur.yml` — см. ниже.

## Публикация: воркфлоу

Публикация автоматическая. Открываете **Actions → Update AUR package → Run
workflow**, вводите версию (например `2.0.4.2.5`) и жмёте запуск.

![workflow](https://github.com/Cross-Cleaner/Cross-Cleaner/actions/workflows/aur.yml/badge.svg)

Воркфлоу:

1. проверяет, что введённая строка — корректная версия для pacman;
2. проверяет, что тег `v<версия>` существует в репозитории;
3. скачивает архив тега и считает sha256;
4. проверяет, что архив распаковывается в каталог, который ожидает `PKGBUILD`
   (иначе об этом узнаёшь только через 40 минут компиляции);
5. клонирует репозиторий AUR и копирует туда `PKGBUILD`, `LICENSE`, `.gitignore`;
6. переписывает четыре строки между маркерами `AUR-VERSION` — версию и checksum;
7. перегенерирует `.SRCINFO` и проверяет, что версия в нём обновилась;
8. коммитит и пушит в `master`.

### Что нужно настроить один раз

Секрет репозитория `AUR_SSH_PRIVATE_KEY` — приватная часть ключа, который
зарегистрирован в аккаунте AUR (см. «Ручная публикация» ниже, шаг с
`ssh-keygen`): **Settings → Secrets and variables → Actions → New repository
secret**.

Публичный ключ при этом уже должен быть в профиле AUR: **My Account → Add SSH
Key**.

### Галочка dry run

У воркфлоу есть вход `dry_run`: он прогоняет все проверки, показывает diff и
останавливается перед `git push`. Стоит начинать с него — он бесплатно ловит
ошибку в версии до того, как что-то уедет в AUR.

### Что НЕ делает воркфлоу

Не трогает `pkgrel`. Он всегда 1, потому что версия привязана к одному тегу, а
новая версия — это новый `pkgver`. Поднять `pkgrel` нужно руками в этом
каталоге, если меняется сам пакет, а не приложение.

## Ручная публикация

Если воркфлоу недоступен — например, до настройки секрета — пакет можно
залить руками. Кнопки «Submit Package» на aur.archlinux.org **нет**: AUR
перешёл на git по SSH.

### 1. Аккаунт и SSH-ключ

Зарегистрируйтесь на https://aur.archlinux.org/passwd/. Создайте отдельный
ключ — основной GitHub-ключ переиспользовать не стоит:

```bash
ssh-keygen -f ~/.ssh/aur
```

Публичный ключ (`~/.ssh/aur.pub`) вставьте в профиль AUR. В `~/.ssh/config`:

```
Host aur.archlinux.org
  IdentityFile ~/.ssh/aur
  User aur
```

### 2. Клонировать и запушить

```bash
git clone ssh://aur@aur.archlinux.org/cross-cleaner.git /tmp/aur-cross-cleaner
cd /tmp/aur-cross-cleaner
cp /home/roman/Documents/GitHub/Cross-Cleaner/packaging/aur/{PKGBUILD,LICENSE,.gitignore} .

# проставить версию и checksum в блок между маркерами, затем:
makepkg --printsrcinfo -p PKGBUILD > .SRCINFO
git add PKGBUILD .SRCINFO LICENSE .gitignore
git commit -m "cross-cleaner <версия>"
git push
```

**Требования AUR, которые ломают пуш:**

- Пуш только в ветку `master`. Если ветка называется иначе — `git branch -M master`.
- **AUR отклоняет non-fast-forward.** Локальная история не должна расходиться с
  удалённой: если расходилась, `--force` не поможет, серверный хук его отклонит.
  Сначала `git pull --rebase origin master`, потом push.
- `PKGBUILD` и `.SRCINFO` должны быть в одном коммите. Забыли `.SRCINFO` —
  `git commit --amend --add .SRCINFO`, а не новый коммит.
- `LICENSE` обязателен: пакеты без лицензии не промотируются в официальные
  репозитории.
- Имя и email коммитов берутся из глобального git-конфига, и после пуша их
  сменить почти невозможно. Для другого авторства задайте их локально до
  коммита: `git config user.name "..." && git config user.email "..."`.

## Требования для сборки

**`base-devel` должен быть установлен.** `rust` и `cargo` входят в
`base-devel`, и AUR-хелперы их **не ставят автоматически** — сборка падает с
`cargo: command not found`. Это давнее соглашение AUR: пакеты предполагают
наличие base-devel в среде сборки.

```bash
sudo pacman -S --needed base-devel
```

Остальные зависимости (`icoutils`, `desktop-file-utils`, `appstream`,
`alsa-lib`, `mesa`, `wayland`) хелпер подтянет сам — они указаны в `makedepends`,
в отличие от `rust`.

**Нужен доступ к crates.io.** Cargo скачивает ~640 крейтов с `index.crates.io` /
`static.crates.io` плюс git-зависимость `gpu-allocator` с GitHub. В сетях, где
crates.io недоступен, сборка падает с
`SSL connect error (Recv failure: Connection reset by peer)` — это не ошибка
пакета. Обходится VPN либо зеркалом реестра в `~/.cargo/config.toml`.

## Проверка перед отправкой

```bash
bash -n PKGBUILD                      # синтаксис
makepkg --printsrcinfo -p PKGBUILD   # метаданные
makepkg -s --noconfirm               # полная сборка
makepkg --packagelist                # какие файлы попадут в пакет
```

Полная сборка долгая: ~640 крейтов плюс `lto = true`, `codegen-units = 1`,
`opt-level = "z"` из workspace-профиля. Это настройка upstream, а не этого
PKGBUILD.

## Особенности пакета

**Версия привязана к тегу, а не к ветке.** Источник — архив тега
(`.../archive/refs/tags/v${pkgver}.tar.gz`), а не `git+`. VCS-источник всегда
идёт за default-веткой, и каждый пользователь собрал бы то, что в `main`
лежало на момент сборки, а не тот релиз, который опубликован. С архивом
версия ниже называет ровно один коммит. Побочный эффект: имя
`cross-cleaner` без суффикса `-git` теперь корректно по правилам AUR.

**Функции `pkgver()` нет.** Раньше она считала версию через `git describe`, и
это делало пакет непригодным: значение всегда расходилось с тем, что показано в
веб-интерфейсе AUR.

**`--locked` не используется.** `Cargo.lock` в релизных тегах устарел
относительно манифестов в тех же тегах: в `v2.0.4.2.5` крейты объявляют
`version = "2.0.4"`, а lock-файл помнит `2.0.1`, поэтому cargo требует
перезаписи lock и `--locked` обрывает сборку до компиляции. То, ради чего
`--locked` добавлялся, всё равно держится: lock уже фиксирует ревизию
`gpu-allocator` из `Cargo.toml`'s `[patch.crates-io]`, и cargo не пересматривает
записи, которые удовлетворяют манифестам. Релизные воркфлоу самого проекта
`--locked` тоже не передают.

**Снимаются флаги компилятора makepkg.** `build()` делает `unset CFLAGS
CXXFLAGS CPPFLAGS FCFLAGS FFLAGS ARFLAGS LDFLAGS LTOFLAGS RUSTFLAGS`. Arch
экспортирует `CFLAGS` в окружение сборки, их подхватывает крейт `cc` и
передаёт компилятору, которым `ring`'s build.rs собирает крипто-ядро, после
чего финальная линковка теряет `-l` для этого архива и падает на всех символах
ring:

```
ld.lld: error: undefined symbol: ring_core_0_17_14__x25519_sc_mask
```

Сама ring при этом собрана верно: build-скрипт выдаёт и `rustc-link-lib`, и
`rustc-link-search`, а в архиве лежат все 157 символов. Проверено отсечением на
уменьшенном крейте (`ureq → rustls → ring`): без `CFLAGS` собирается, с ними —
нет, а LTO, `RUSTFLAGS`, `LDFLAGS` и остальное окружение makepkg ни при чём.

**`prepare()` идемпотентна.** Иконка извлекается во временный каталог, а не
прямо в `$srcdir`: makepkg переиспользует существующий `$srcdir`, если изменился
только PKGBUILD, и старая схема падала с `mv: ... are the same file`.

**Три бинарника.** `cross-cleaner` (оконное приложение), `cross-cleaner-tui`
(терминальное) и `cross-cleaner-cli` (для скриптов). `cargo` называет их по
именам пакетов (`desktop`, `tui`, `cli`), в PKGBUILD они переименовываются,
чтобы `Exec=` в desktop-файлах не зависел от конкретного релиза.

**`--no-default-features` везде.** Путь самообновления выключен намеренно:
версией установленного пакета управляет пакетный менеджер. Так же поступает
`.deb`-сборка — см. `packaging/linux/nfpm.yaml`.

**Зависимости.** Список `depends` перенесён из `nfpm.yaml`, где он тоже
поддерживается вручную: winit через `x11-dl` открывает весь X11-стек через
`dlopen`, поэтому эти библиотеки не видны ни в `ldd`, ни в проверках
зависимостей, но без них окно не откроется.

**Иконка.** `crates/winicon/assets/icon.ico` содержит один кадр 96×96, и
`icotool` извлекает его как есть. В отличие от AppImage-сборки, здесь ничего не
растягивается под 128 и 256 — установлен единственный настоящий размер. Чтобы
добавить остальные, нужен мастер-PNG ≥512px в `crates/winicon/assets`.

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

## Известная проблема upstream

В релизных тегах `Cargo.lock` не соответствует манифестам: версии крейтов в
`Cargo.toml` подняты до `2.0.4`, а lock-файл остался на `2.0.1`. Пока это так,
`cargo build --locked` не работает ни на одном теге. Лечится регенерацией
lock-файла в релизном воркфлоу — например, добавлением
`cargo update --workspace` в задачу `formating_code` перед коммитом.