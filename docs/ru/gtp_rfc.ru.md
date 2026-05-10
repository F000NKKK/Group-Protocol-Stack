# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Text Protocol (GTP) over GBP

## Аннотация
Документ задает GTP — протокол групповых текстовых и бинарных сообщений поверх GBP (`StreamType=2`).

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1.  Введение
GTP обеспечивает упорядоченную и надежную доставку сообщений в общей MLS-группе.

## 2.  Соглашения
Термины BCP 14 трактуются по [RFC2119] и [RFC8174].

## 3.  Привязка к GBP
GTP-сообщения MUST передаваться только при `stream_type=2`.

## 4.  Формат сообщения
```
GTPMessage {
  uint64 message_id;
  uint32 sender_id;
  uint64 timestamp_ms;
  uint32 request_id;
  uint8  flags;
  uint8  content_type;
  uint32 content_length;
  bytes  content;
}
```

Flags:
- `0x01` urgent
- `0x02` ephemeral
- `0x04` persistent

ContentType:
- `0` plain
- `1` markdown
- `2` binary
- `3` attachment_ref

## 5.  Доставка
Для чата отправитель SHOULD использовать `O|R|A`.
Повтор `(sender_id, message_id)` MUST считаться идемпотентным дубликатом.

## 6.  Вложения
Вложения SHOULD выноситься в отдельный канал/чанки и MUST иметь ссылку на parent message и метаданные целостности.

## 7.  Ресинхронизация
Клиент MUST передать watermark по KnownMessageID. Узел SHOULD воспроизвести пропущенные сообщения в порядке в рамках политики хранения.

## 8.  Ошибки
- `ERR_GTP_BAD_LENGTH`
- `ERR_GTP_UNSUPPORTED_CONTENT_TYPE`
- `ERR_GTP_DUPLICATE_MESSAGE`
- `ERR_GTP_POLICY_REJECTED`

## 9.  Схемы сообщений

### 9.1 CBOR
```
{
  "mid": uint,
  "sid": uint,
  "ts": uint,
  "rid": uint,
  "fl": uint,
  "ct": uint,
  "len": uint,
  "body": bstr
}
```

### 9.2 Protobuf
```proto
syntax = "proto3";
package gtp;

message GTPMessage {
  uint64 message_id = 1;
  uint32 sender_id = 2;
  uint64 timestamp_ms = 3;
  uint32 flags = 4;
  uint32 content_type = 5;
  uint32 content_length = 6;
  bytes content = 7;
}
```

### 9.3 FlatBuffers
```fbs
namespace gtp;

table GTPMessage {
  message_id:ulong;
  sender_id:uint;
  timestamp_ms:ulong;
  flags:ubyte;
  content_type:ubyte;
  content_length:uint;
  content:[ubyte];
}

root_type GTPMessage;
```

## 10.  IANA Considerations
Дополнительных действий IANA не требуется.

## 11.  Security Considerations
Реализация MUST проверять авторизацию, политику хранения и replay-защиту до применения побочных эффектов.

## 12. References
### 12.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
