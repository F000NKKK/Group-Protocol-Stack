# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Stack Serialization and Interoperability Profile

## Аннотация
Документ определяет общие правила сериализации и интероперабельности для семейства GBP (`GBP`, `GAP`, `GTP`, `GSP`).

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1.  Введение
Профиль требуется для совместимости форматов между реализациями на разных языках и платформах.

## 2.  Соглашения
Требования BCP 14 трактуются по [RFC2119] и [RFC8174].

## 3.  Базовые зависимости
- Безопасность: MLS [RFC9420], TLS 1.3 [RFC8446].
- Транспорт: QUIC [RFC9000].
- Медиа: Opus [RFC6716], RTP [RFC3550], SRTP [RFC3711].
- Traversal: ICE [RFC8445], STUN [RFC5389], TURN [RFC8656].

## 4.  Межпротокольные требования
- Узлы MUST отклонять некорректные длины.
- Узлы MUST отклонять ошибки аутентификации.
- Узлы MUST обеспечивать монотонный рост epoch.
- Управляющие потоки MUST использовать надежную доставку.
- Replay при resync SHOULD быть детерминированным.

## 5.  Общие правила кодирования
- По умолчанию целые поля беззнаковые.
- Бинарные поля: `bstr` (CBOR), `bytes` (Protobuf), `[ubyte]` (FlatBuffers).
- Неизвестные enum-значения MUST обрабатываться безопасно.

## 6.  Общие перечисления
### 6.1 StreamType
- `0` control
- `1` audio
- `2` text
- `3` signal

### 6.2 GBP Flags
- `0x0001` ordered (`O`)
- `0x0002` reliable (`R`)
- `0x0004` ack requested (`A`)
- `0x0008` system (`S`)
- `0x0010` critical extension (`C`)

### 6.3 Диапазоны Error Code
- `0x0000-0x0FFF` GBP
- `0x1000-0x1FFF` GAP
- `0x2000-0x2FFF` GTP
- `0x3000-0x3FFF` GSP
- `0xF000-0xFFFF` Private use

### 6.4 PayloadCodec
Идентифицирует кодек субпротокольного сообщения внутри AEAD-запечатанной полезной нагрузки.
Передаётся в поле `pf` (payload format) кадра GBP; отсутствие `pf` MUST трактоваться как `0`.

- `0` CBOR (по умолчанию; поле `pf` опускается при значении 0 для обратной совместимости с pre-1.5 узлами)
- `1` Protobuf
- `2` FlatBuffers
- `3–127` стандартные действия
- `128–255` частное использование

## 7.  Общий Protobuf Envelope (опционально)
```proto
syntax = "proto3";
package gbpstack;

message Envelope {
  uint32 version = 1;
  bytes group_id = 2;
  uint64 epoch = 3;
  uint32 stream_type = 4;
  uint32 stream_id = 5;
  uint32 flags = 6;
  bytes payload = 7;
}
```

## 8.  Общий FlatBuffers Envelope (опционально)
```fbs
namespace gbpstack;

table Envelope {
  version:ubyte;
  group_id:[ubyte];
  epoch:ulong;
  stream_type:ubyte;
  stream_id:uint;
  flags:ushort;
  payload:[ubyte];
}

root_type Envelope;
```

## 9.  Общий CBOR Envelope (опционально)
```
{
  "v": uint,
  "gid": bstr,
  "ep": uint,
  "st": uint,
  "sid": uint,
  "fl": uint,
  "pl": bstr
}
```

## 10.  Validation Checklist
- Проверка версии.
- Проверка членства и авторизации.
- Проверка epoch и gate для resync.
- Проверка длины и схемы полезной нагрузки.
- Генерация ACK/NACK по политике доставки.

## 11.  IANA Considerations
Новых реестров не вводится; используются реестры GBP.

## 12.  Security Considerations
Схемная совместимость не заменяет криптографическую проверку. Получатель MUST валидировать криптографию и права до применения побочных эффектов.

## 13. References
### 13.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC8446] Rescorla, E., "The Transport Layer Security (TLS) Protocol Version 1.3".
- [RFC8949] Bormann, C. and P. Hoffman, "Concise Binary Object Representation (CBOR)".
- [RFC9000] Iyengar, J. and M. Thomson, "QUIC: A UDP-Based Multiplexed and Secure Transport".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
