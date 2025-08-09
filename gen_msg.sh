#!/usr/bin/env bash
set -euo pipefail

# Параметры (настройте под вашу схему)
PROTO_FILE="example.proto"        # путь к .proto
MESSAGE_TYPE="example.Person"     # полное имя сообщения: <package>.<Message>
OUT_BIN="proto_message.bin"       # выходной бинарный файл

# Если хотите читать JSON из файла, укажите путь здесь:
# JSON_FILE="data.json"
# Иначе будет использован встроенный пример:
JSON_DATA='{
  "name": "John Doe",
  "id": 123456,
  "email": "johndoe@example.com"
}'

# Проверки окружения
command -v protoc >/dev/null 2>&1 || { echo "Ошибка: protoc не найден в PATH"; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "Ошибка: python3 не найден в PATH"; exit 1; }
[[ -f "$PROTO_FILE" ]] || { echo "Ошибка: не найден proto-файл: $PROTO_FILE"; exit 1; }

# Получение JSON: из файла или из встроенной переменной
if [[ "${JSON_FILE:-}" != "" ]]; then
  [[ -f "$JSON_FILE" ]] || { echo "Ошибка: не найден JSON-файл: $JSON_FILE"; exit 1; }
  JSON_INPUT="$(cat "$JSON_FILE")"
else
  JSON_INPUT="$JSON_DATA"
fi

# Генерация textproto из JSON при помощи Python (JSON передаём через переменную окружения)
export JSON_INPUT
TEXTPROTO="$(python3 -c '
import json, os, sys

try:
    data = json.loads(os.environ["JSON_INPUT"])
except Exception as e:
    print(f"Ошибка чтения JSON: {e}", file=sys.stderr)
    sys.exit(1)

def must_have(key):
    if key not in data:
        print(f"Ошибка: отсутствует поле {key!r} в JSON", file=sys.stderr)
        sys.exit(1)
    return data[key]

name = must_have("name")
idv = must_have("id")
email = must_have("email")

if not isinstance(name, str):
    print("Ошибка: name должен быть строкой", file=sys.stderr); sys.exit(1)
if not isinstance(idv, int):
    print("Ошибка: id должен быть целым числом (int)", file=sys.stderr); sys.exit(1)
if not isinstance(email, str):
    print("Ошибка: email должен быть строкой", file=sys.stderr); sys.exit(1)

# Экранируем кавычки в строках
def esc(s: str) -> str:
    return s.replace("\\\\", "\\\\\\\\").replace("\"", "\\\"")

print(f"name: \"{esc(name)}\"")
print(f"id: {idv}")
print(f"email: \"{esc(email)}\"")
')"

# Кодирование в бинарный Protobuf через protoc --encode
INCLUDE_DIR="$(dirname "$PROTO_FILE")"
if [[ "$INCLUDE_DIR" == "." ]]; then
  protoc --encode="$MESSAGE_TYPE" "$PROTO_FILE" <<< "$TEXTPROTO" > "$OUT_BIN"
else
  protoc -I"$INCLUDE_DIR" --encode="$MESSAGE_TYPE" "$PROTO_FILE" <<< "$TEXTPROTO" > "$OUT_BIN"
fi

echo "Protobuf message generated successfully: $OUT_BIN"

# (Опционально) Верификация обратным декодированием:
# if [[ "$INCLUDE_DIR" == "." ]]; then
#   protoc --decode="$MESSAGE_TYPE" "$PROTO_FILE" < "$OUT_BIN"
# else
#   protoc -I"$INCLUDE_DIR" --decode="$MESSAGE_TYPE" "$PROTO_FILE" < "$OUT_BIN"
# fi