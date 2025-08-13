### **НА МАКЕ ПЕРЕД ЗАПУСКОМ ВЫПОЛНИ (что бы достать из карантина):** 
 `xattr -r -d com.apple.quarantine rkui.app`

# rkui

rkui — кроссплатформенное настольное приложение (Tauri v2 + Rust + React/Vite) для чтения и анализа сообщений Apache Kafka с возможностью декодирования Protobuf.

Основные возможности:
- Подключение к Kafka (SSL по умолчанию; SASL — опционально через фичу сборки).
- Просмотр топиков и партиций, получение сообщений из выбранных партиций.
- Фильтрация и потоковая подгрузка записей.
- Декодирование сообщений по .proto-схемам (динамически, без генерации кода).

Стек:
- Backend: Rust, Tauri v2, rdkafka, tokio, serde.
- Frontend: React + Vite + TypeScript (директория `web`).


## Требования

Общие:
- Rust (stable)
- Node.js 18+ (рекомендуется 20 LTS) и npm
- Tauri CLI: `cargo install tauri-cli`

Linux (Ubuntu):
- `sudo apt-get update`
- `sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf librdkafka-dev`

macOS:
- Xcode Command Line Tools: `xcode-select --install`
- (Опционально) сертификаты для подписи, если планируете распространение за пределами CI. Без них будут собираться неподписанные артефакты (ad-hoc).
- Важно: начиная с этой версии, сборка по умолчанию не требует `libsasl2` (фича SASL отключена). Если вам нужна аутентификация SASL, собирайте с фичей `with-sasl` и установите `cyrus-sasl` (Homebrew: `brew install cyrus-sasl`).
- OpenSSL: для macOS используется vendored-сборка (`openssl-sys` с фичей `vendored`), поэтому переменная `OPENSSL_DIR` не требуется. При желании можно переопределить и использовать системный OpenSSL, установив `OPENSSL_DIR` или `OPENSSL_NO_VENDOR=1` перед сборкой.


## Установка зависимостей фронтенда

Фронтенд находится в каталоге `web`:

```bash
npm ci --prefix web
```

Это нужно выполнить один раз (и при изменении зависимостей фронта).


## Локальная разработка

```bash
# Убедитесь, что фронтенд зависимости установлены
npm ci --prefix web

# Запуск приложения в dev-режиме (Tauri сам запустит Vite dev-сервер)
cargo tauri dev
```

По умолчанию Tauri использует настройки из `tauri.conf.json`. В конфиге уже указаны команды сборки фронтенда из каталога `web`.


## Сборка релизной версии

### Варианты фич
- По умолчанию: без SASL (не требуется `libsasl2`).
- С SASL: добавьте фичу `with-sasl` — пример: `cargo build --features with-sasl` или `cargo tauri build --features with-sasl`.

```bash
# Убедитесь, что фронтенд зависимости установлены
npm ci --prefix web

# Сборка релизной версии
cargo tauri build
```

Результаты сборки появляются в `target/release` и установочные пакеты — в `target/release/bundle/*` (AppImage/DEB на Linux, .app/.dmg на macOS и т.д., в зависимости от платформы).


## Работа с Protobuf (пример)

В репозитории есть пример скрипта для генерации бинарного сообщения Protobuf: `./gen_msg.sh` (использует `protoc`). Он собирает `proto_message.bin` по схемам `example.proto` и `address.proto` и может использоваться для тестирования декодера.

```bash
./gen_msg.sh
```


## Иконки приложения

Для сборки пакетов нужен набор иконок. Вы можете сгенерировать их из одного SVG:

```bash
# из каталога web
npm run tauri:icon --prefix web

# или из корня
cargo tauri icon -o icons assets/icon.png
```

После генерации в корне появится каталог `icons` с необходимыми файлами. Путь к иконке/иконкам настраивается в `tauri.conf.json` (секция `bundle.icon`).


## CI/CD: релиз по тэгу

При пуше тэга вида `v*` (например, `v0.1.0`) GitHub Actions собирает релизные артефакты для Linux, macOS и Windows и публикует их в GitHub Releases.

Шаги:
- Убедитесь, что в репозитории включён GitHub Actions.
- Создайте тэг и отправьте в удалённый репозиторий:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Action автоматически:
- Установит зависимости среды (Rust/Node и системные библиотеки для Linux; инструменты сборки для macOS/Windows).
- Соберёт фронтенд и Tauri-приложение.
- Создаст релиз и приложит артефакты (исполняемые файлы/пакеты для Linux, macOS и Windows).


## Windows

Ниже — проверенные шаги для сборки и нативного запуска под Windows (MSVC toolchain).

Примечание про SASL: по умолчанию SASL выключён, поэтому `libsasl2`/Cyrus SASL не требуется. Если вам нужна SASL-аутентификация, собирайте с фичей `with-sasl` и обеспечьте наличие зависимостей (например, через vcpkg).

Требования:
- Visual Studio 2022 Build Tools с компонентами для C++ (MSVC, Windows 10/11 SDK). Установить через Visual Studio Installer.
- Rust (stable) с профилем MSVC (обычный rustup на Windows ставит msvc по умолчанию).
- Node.js 20 LTS и npm.
- Tauri CLI: `cargo install tauri-cli`.
- Microsoft Edge WebView2 Runtime: https://developer.microsoft.com/en-us/microsoft-edge/webview2/ (чаще всего уже установлен, но лучше проверить).
- vcpkg (для сборки зависимостей rdkafka + OpenSSL, zlib, zstd):
  - PowerShell (от админа желательно):
    - `git clone https://github.com/microsoft/vcpkg C:\vcpkg`
    - `C:\vcpkg\bootstrap-vcpkg.bat`
    - Установить необходимые пакеты (x64):
      - `C:\vcpkg\vcpkg.exe install librdkafka:x64-windows zlib:x64-windows zstd:x64-windows openssl:x64-windows`

Переменные окружения (PowerShell):
```pwsh
$env:VCPKG_ROOT = "C:\\vcpkg"
$env:VCPKGRS_DYNAMIC = "1"        # линковка с динамическими библиотеками из vcpkg
$env:VCPKGRS_TRIPLET = "x64-windows"
# Рекомендуется добавить бинарники зависимостей в PATH для dev-режима:
$env:Path = "C:\\vcpkg\\installed\\x64-windows\\bin;" + $env:Path
```

Установка зависимостей фронтенда:
```pwsh
npm ci --prefix web
```

Запуск в режиме разработки:
```pwsh
# В новой сессии PowerShell установите env-переменные (см. выше)
# затем:
cargo tauri dev
```

Сборка релизной версии:
```pwsh
# В новой сессии PowerShell установите env-переменные (см. выше)
# затем:
cargo tauri build
```

Где найти результаты:
- Исполняемый файл и установочные пакеты появятся в `target\release` и `target\release\bundle\*`.

Примечания:
- rdkafka на Windows требует присутствия соответствующих DLL (openssl, zlib, zstd, librdkafka) во время выполнения. В dev-режиме убедитесь, что `C:\vcpkg\installed\x64-windows\bin` в PATH. Пакетный установщик, собираемый Tauri, обычно включает необходимые зависимости.
- Если используете прокси/корпоративную сеть, убедитесь, что `vcpkg` может скачивать и собирать пакеты.
- OpenSSL: на Windows по умолчанию включена vendored-сборка (`openssl-sys` с `vendored`), так что `OPENSSL_DIR` не требуется. Если хотите использовать OpenSSL из vcpkg, задайте `OPENSSL_NO_VENDOR=1` и установите `openssl:x64-windows`.
- Если возникнут проблемы со сборкой OpenSSL, убедитесь, что используется MSVC toolchain (не gnu) и установлены компоненты Windows SDK.
