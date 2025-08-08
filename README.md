# rkui

Этот проект использует Tauri v2. Для сборки установочного пакета необходим набор иконок, перечисленных в `tauri.conf.json`.

## Генерация набора иконок через Tauri CLI

Tauri предоставляет встроенную команду генерации иконок из одного SVG/PNG исходника.

Требования:
- Установлен Tauri CLI: `cargo install tauri-cli` (или следуйте официальной инструкции: https://tauri.app/v1/guides/getting-started/prerequisites/)
- Rust и Node уже установлены (как для обычной разработки с Tauri).

В проект уже добавлен базовый файл иконки:
- `assets/icon.svg`

И добавлен npm-скрипт для генерации из директории `web`:

```bash
npm run tauri:icon --prefix web
```

Эта команда выполнит:

```bash
cargo tauri icon -o icons assets/icon.svg
```

В результате в корне проекта появится папка `icons` с требуемыми файлами:
- `icons/icon.icns`
- `icons/icon.ico`
- `icons/128x128.png`
- `icons/256x256.png`
- `icons/512x512.png`

Пути к этим файлам уже прописаны в `tauri.conf.json` в секции `bundle.icon`.

### Примечания
- Вместо `assets/icon.svg` вы можете использовать свой PNG или SVG файл (рекомендуется 512×512 и больше), просто поменяйте путь в команде: `cargo tauri icon -o icons path/to/your_icon.(png|svg)`.
- Команду можно запускать повторно при смене иконки — файлы будут перегенерированы.
- Если вы работаете не из папки `web`, можете запустить ту же команду из корня:

```bash
cargo tauri icon -o icons assets/icon.svg
```

После генерации можете собрать приложение:

```bash
# dev-режим
cargo tauri dev

# release-сборка
cargo tauri build
```
