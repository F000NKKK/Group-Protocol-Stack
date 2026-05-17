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
- `ERR_GSP_PRECONDITION_FAILED`

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
  uint32 role_claim = 4;
  uint32 args_length = 5;
  bytes args_cbor = 6;
}
```

### 9.3 FlatBuffers
```fbs
namespace gsp;

table GSPSignal {
  signal_type:ushort;
  request_id:uint;
  sender_id:uint;
  role_claim:uint;
  args_length:uint;
  args:[ubyte];
}

root_type GSPSignal;
```

### 9.4 CBOR-схемы аргументов по типу сигнала

Реализации MUST валидировать поле `args` в соответствии со следующими
схемами. Сигналы без аргументов MUST передаваться с пустым `args`
(`args_length = 0`). Сигналы с обязательной схемой MUST NOT приниматься при
пустом `args` или несоответствии схеме — получатель MUST вернуть
`ERR_GSP_BAD_SCHEMA`.

| Тип сигнала | Код | Схема CBOR `args` | Ключи |
|------------|-----|-------------------|-------|
| JOIN | 100 | *(пусто)* | — |
| LEAVE | 101 | *(пусто)* | — |
| ROLE_CHANGE | 102 | `{0: target_member_id, 1: new_role_id}` | 0=uint цель, 1=uint роль |
| MUTE | 200 | `{0: target_member_id}` | 0=uint цель |
| UNMUTE | 201 | `{0: target_member_id}` | 0=uint цель |
| STREAM_START | 300 | `{0: stream_type}` | 0=uint тип потока |
| STREAM_STOP | 301 | `{0: stream_type}` | 0=uint тип потока |
| CODEC_UPDATE | 400 | `{0: codec_id}` | 0=uint идентификатор кодека |

Все ключи карты — CBOR unsigned integer (major type 0). Значения — unsigned
integer. Формат CBOR карты MUST быть definite-length.

Пример — MUTE для участника 3 (CBOR hex: `A1 00 03`):
```
A1       -- map(1)
   00    -- ключ: 0 (target_member_id)
   03    -- значение: 3
```

Пример — ROLE_CHANGE для участника 5, новая роль 2 (CBOR hex: `A2 00 05 01 02`):
```
A2       -- map(2)
   00    -- ключ: 0 (target_member_id)
   05    -- значение: 5
   01    -- ключ: 1 (new_role_id)
   02    -- значение: 2
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
