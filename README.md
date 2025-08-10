# rkui

rkui — кроссплатформенное настольное приложение (Tauri v2 + Rust + React/Vite) для чтения и анализа сообщений Apache Kafka с возможностью декодирования Protobuf.

Основные возможности:
- Подключение к Kafka (SASL/SSL поддерживается rdkafka).
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
cargo tauri icon -o icons assets/icon.svg
```

После генерации в корне появится каталог `icons` с необходимыми файлами. Путь к иконке/иконкам настраивается в `tauri.conf.json` (секция `bundle.icon`).


## CI/CD: релиз по тэгу

При пуше тэга вида `v*` (например, `v0.1.0`) GitHub Actions собирает релизные артефакты для Linux и macOS и публикует их в GitHub Releases.

Шаги:
- Убедитесь, что в репозитории включён GitHub Actions.
- Создайте тэг и отправьте в удалённый репозиторий:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Action автоматически:
- Установит зависимости среды (Rust/Node и системные библиотеки для Linux).
- Соберёт фронтенд и Tauri-приложение.
- Создаст релиз и приложит артефакты (исполняемые файлы/пакеты для macOS и Linux).
