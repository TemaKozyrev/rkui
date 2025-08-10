#!/usr/bin/env bash
set -euo pipefail

# Параметры (настройте под вашу схему)
PROTO_FILE="example.proto"        # путь к .proto
MESSAGE_TYPE="example.Person"     # полное имя сообщения: <package>.<Message>
OUT_BIN="proto_message.bin"       # выходной бинарный файл
ADDRESS_PROTO="address.proto"     # путь к address.proto

# Захардкоженные параметры для Person и Address
NAME="Marie Doe"
ID=123456
EMAIL="johndoe@example.com"
CITY="New York"
STREET="Broadway"

# Экранируем кавычки в строках
esc() {
    echo "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

TEXTPROTO=$(cat <<EOF
name: "$(esc "$NAME")"
id: $ID
email: "$(esc "$EMAIL")"
address {
  city: "$(esc "$CITY")"
  street: "$(esc "$STREET")"
}
EOF
)

# Кодирование в бинарный Protobuf через protoc --encode с указанием пути к .proto файлу
INCLUDE_DIR="$(dirname "$PROTO_FILE")"
ADDR_INCLUDE_DIR="$(dirname "$ADDRESS_PROTO")"
if [[ "$INCLUDE_DIR" == "." && "$ADDR_INCLUDE_DIR" == "." ]]; then
  protoc --proto_path=. --encode="$MESSAGE_TYPE" "$PROTO_FILE" <<< "$TEXTPROTO" > "$OUT_BIN"
else
  protoc --proto_path="$ADDR_INCLUDE_DIR:$INCLUDE_DIR" --encode="$MESSAGE_TYPE" "$PROTO_FILE" <<< "$TEXTPROTO" > "$OUT_BIN"
fi

echo "Protobuf message generated successfully: $OUT_BIN"
