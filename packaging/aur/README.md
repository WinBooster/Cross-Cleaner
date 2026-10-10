# Cross Cleaner — AUR

Файлы для публикации Cross Cleaner в Arch User Repository. Всё в этом каталоге
предназначено для отдельного git-репозитория на AUR — в основной репозиторий
проекта они попасть не должны.

## Состав

| Файл | Назначение |
|---|---|
| `PKGBUILD` | Скрипт сборки пакета |
| `.SRCINFO` | Метаданные для веб-интерфейса AUR |

## Публикация

### 1. Создать репозиторий на AUR

На https://aur.archlinux.org нажмите «Submit Package» → «Create new package»,
имя: `cross-cleaner`. Git-репозиторий создастся пустым.

### 2. Залить файлы

```bash
cd packaging/aur
git init
git remote add origin ssh://aur@aur.archlinux.org/cross-cleaner.git
git add PKGBUILD .SRCINFO
git commit -m "Initial AUR package"
git push -u origin master
```

После пуша пакет появится в AUR через несколько минут (иногда до часа) и станет
доступен через ваш обычный AUR-клиент:

```bash
paru -S cross-cleaner
```

`.SRCINFO` генерируется из `PKGBUILD`, поэтому после любой правки `PKGBUILD`
перегенерируйте его и коммитьте оба файла вместе:

```bash
makepkg --printsrcinfo -p PKGBUILD > .SRCINFO
git commit -am "..." && git push
```

## Проверка перед отправкой

```bash
# Синтаксис и метаданные
bash -n PKGBUILD
makepkg --printsrcinfo -p PKGBUILD

# Полная сборка (долго: ~636 крейтов, lto + opt-level=z)
makepkg -s --noconfirm

# Проверка зависимостей: ругается на то, чего не хватает в системе
makepkg --syncdeps
```

## Особенности пакета

**Собирается из git, а не из tarball.** `source` указывает на репозиторий, а
версия вычисляется через `git describe`. Новый тег upstream подхватывается сам,
orphanage увидит изменившуюся версию — руками править ничего не нужно.

**Три бинарника.** `cross-cleaner` (оконное приложение), `cross-cleaner-tui`
(терминальное) и `cross-cleaner-cli` (для скриптов). `cargo` называет их по
именам пакетов (`desktop`, `tui`, `cli`), в PKGBUILD они переименовываются, чтобы
`Exec=` в desktop-файлах не зависел от конкретного релиза.

**`--no-default-features` везде.** Путь самообновления (проверка GitHub на новый
релиз и замена собственного исполняемого файла) выключен намеренно: версией
установленного пакета управляет пакетный менеджер, и приложение не должно ему
мешать. Так же поступает `.deb`-сборка — см. `packaging/linux/nfpm.yaml`.

**Зависимости.** Список `depends` перенесён из `nfpm.yaml`, где он тоже
поддерживается вручную: winit через `x11-dl` открывает весь X11-стек через
`dlopen`, поэтому эти библиотеки не видны ни в `ldd`, ни в
`ldd`-эквивалентных проверках, но без них окно не откроется.

**Иконка.** `crates/winicon/assets/icon.ico` содержит один кадр 96×96, и
`icotool` извлекает его как есть. В отличие от AppImage-сборки, здесь ничего не
растягивается под 128 и 256 — установлен единственный настоящий размер. Чтобы
добавить остальные, нужен мастер-PNG ≥512px в `crates/winicon/assets`.

**Метаданные AppStream.** `packaging/linux/cross-cleaner.appdata.xml` содержит
плейсхолдеры `%%VERSION%%` и `%%DATE%%`, которые в релизе подставляет workflow.
Здесь то же самое делает `prepare()` — по вычисленному `pkgver` и дате коммита.