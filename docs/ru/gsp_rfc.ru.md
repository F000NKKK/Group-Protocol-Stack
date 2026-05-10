# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Signaling Protocol (GSP) over GBP

## Аннотация
Документ задает GSP — протокол управляющей сигнализации группы поверх GBP (`StreamType=3`).

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1.  Введение
GSP отвечает за изменение состояния группы, ролей и управление медиапотоками.

## 2.  Соглашения
Ключевые слова BCP 14 трактуются согласно [RFC2119] и [RFC8174].

## 3.  Привязка к GBP
GSP-сообщения MUST передаваться только при `stream_type=3`.
GSP-сообщения MUST использовать надежную доставку.

## 4.  Формат сигнала
```
GSPSignal {
  uint16 signal_type;
  uint32 request_id;
  uint32 sender_id;
  uint32 role_claim;
  uint32 args_length;
  bytes  args;
}
```

`args` MUST быть валидным CBOR для соответствующего `signal_type`.

## 5.  Реестр SignalType
- `100` JOIN
- `101` LEAVE
- `102` ROLE_CHANGE
- `200` MUTE
- `201` UNMUTE
- `300` STREAM_START
- `301` STREAM_STOP
- `400` CODEC_UPDATE

## 6.  Обработка и авторизация
Получатель MUST: дешифровать, проверить права отправителя, валидировать схему аргументов, применить изменения атомарно, вернуть ACK/NACK.

## 7.  Восстановление
Реализации SHOULD хранить недавнюю историю управляющих кадров и MUST поддерживать детерминированный replay.

## 8.  Ошибки
- `ERR_GSP_BAD_SCHEMA`
- `ERR_GSP_UNAUTHORIZED`
- `ERR_GSP_UNKNOWN_SIGNAL`
- `ERR_GSP_DUPLICATE_REQUEST`
- `ERR_GSP_STATE_CONFLICT`

## 9.  Схемы сообщений

### 9.1 CBOR
```
{
  "t": uint,
  "rid": uint,
  "sid": uint,
  "rc": uint,
  "alen": uint,
  "args": any
}
```

### 9.2 Protobuf
```proto
syntax = "proto3";
package gsp;

message GSPSignal {
  uint32 signal_type = 1;
  uint32 request_id = 2;
  uint32 sender_id = 3;
  uint32 args_length = 4;
  bytes args_cbor = 5;
}
```

### 9.3 FlatBuffers
```fbs
namespace gsp;

table GSPSignal {
  signal_type:ushort;
  request_id:uint;
  sender_id:uint;
  args_length:uint;
  args:[ubyte];
}

root_type GSPSignal;
```

## 10.  IANA Considerations
Расширяемость определяется реестром GBP.

## 11.  Security Considerations
Реализация MUST строго связывать контроль доступа с аутентифицированной личностью отправителя и актуальным состоянием ролей.

## 12. References
### 12.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC8949] Bormann, C. and P. Hoffman, "Concise Binary Object Representation (CBOR)".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
