# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Audio Protocol (GAP) over GBP

## Аннотация
Документ задает GAP — подпротокол низколатентного группового аудио поверх GBP (`StreamType=1`).

GAP payload'ы МОГУТ быть дополнительно защищены SFrame E2EE согласно `gbp-sframe.ru.md`. При использовании SFrame GBP-узел передаёт приложению SFrame payload'ы (не сырой Opus); приложение отвечает за шифрование/дешифрование SFrame перед передачей фреймов в `GapClient`.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1.  Введение
GAP передает Opus-кадры в рамках общей защищенной MLS/GBP-сессии.

## 2.  Соглашения
Ключевые слова BCP 14 используются согласно [RFC2119] и [RFC8174].

## 3.  Привязка к GBP
Отправитель MUST использовать только `stream_type=1`.
Получатель MUST отклонять GAP-полезную нагрузку в иных stream type.

## 4.  Ключевая модель
- `MediaMasterKey`
- `MediaSalt`
- `MediaEncryptionKey = HKDF(MediaMasterKey, MediaSalt, "audio/sender")`
- `MediaAuthenticationKey = HKDF(MediaMasterKey, MediaSalt, "audio/auth")`

Производные SRTP-ключи MUST NOT передаваться по сети.

## 5.  Формат полезной нагрузки
```
GAPPayload {
  uint32 media_source_id;
  uint16 rtp_sequence;
  uint64 rtp_timestamp;
  uint32 key_phase;
  bytes  opus_frame;
}
```

`rtp_timestamp` MUST следовать 48 kHz аудио-часам.

## 6.  Обработка
Получатель MUST: дешифровать, валидацировать источник, проверить replay, декодировать Opus.
Получатель SHOULD использовать jitter buffer и MAY отбрасывать опоздавшие кадры.

## 7.  Профиль производительности
- Opus 48 kHz: REQUIRED.
- 20 ms packetization: RECOMMENDED.
- Opus FEC: RECOMMENDED.
- Надежная доставка для разговорного аудио: NOT RECOMMENDED.

## 8.  Ошибки
- `ERR_GAP_BAD_SOURCE_ID`
- `ERR_GAP_DECODE_FAILED`
- `ERR_GAP_AUTH_FAILED`
- `ERR_GAP_REPLAY_DETECTED`
- `ERR_GAP_EPOCH_STALE`
- `ERR_GAP_KEY_PHASE_UNKNOWN`

## 9.  Схемы сообщений

### 9.1 CBOR
```
{
  "msid": uint,
  "seq": uint,
  "ts": uint,
  "kp": uint,
  "opus": bstr
}
```

### 9.2 Protobuf
```proto
syntax = "proto3";
package gap;

message GAPPayload {
  uint32 media_source_id = 1;
  uint32 rtp_sequence = 2;
  uint64 rtp_timestamp = 3;
  uint32 key_phase = 4;
  bytes opus_frame = 5;
}
```

### 9.3 FlatBuffers
```fbs
namespace gap;

table GAPPayload {
  media_source_id:uint;
  rtp_sequence:ushort;
  rtp_timestamp:ulong;
  key_phase:uint;
  opus_frame:[ubyte];
}

root_type GAPPayload;
```

## 9.4  Выбор кодека полезной нагрузки
Кодек GAPPayload передаётся в поле `pf` кадра GBP (см. `gbp_rfc.ru.md` §6.1 и
`schemas.ru.md` §6.4). Использование `pf=2` (FlatBuffers) минимизирует задержку
декодирования для аудиопотоков реального времени.

## 10.  IANA Considerations
Дополнительных действий IANA не требуется.

## 11.  Security Considerations
Реализация MUST обеспечивать replay-защиту и очистку старых ключей при смене epoch.

## 12. References
### 12.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC6716] Valin, J., et al., "Definition of the Opus Audio Codec".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
- `gbp-sframe.ru.md` — SFrame E2EE для GAP аудиопотоков.
