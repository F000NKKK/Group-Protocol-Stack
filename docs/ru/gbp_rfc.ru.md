# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Base Protocol (GBP)

## Аннотация
Этот документ задает Group Base Protocol (GBP) — защищенную мультиплексированную базу групповой связи поверх QUIC и MLS. GBP определяет состояние группы, смену epoch, формат кадров, семантику доставки и процедуры ресинхронизации. Протоколы верхнего уровня (аудио, текст, сигнализация) передаются как типизированные потоки внутри GBP.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1.  Введение
GBP предоставляет единый транспортный и криптографический контур для групповых приложений и не определяет бизнес-смысл полезной нагрузки.

## 2.  Соглашения и термины
Ключевые слова "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT",
"SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY" и
"OPTIONAL" трактуются согласно BCP 14 [RFC2119] [RFC8174], только если
они записаны заглавными буквами.

Термины:
- **GroupID**: глобально уникальный идентификатор группы (64-128 бит).
- **MemberID**: уникальный идентификатор участника в пределах GroupID.
- **Epoch**: поколение ключевого состояния MLS.
- **TransitionID**: монотонный идентификатор перехода.
- **StreamType**: класс протокола верхнего уровня.
- **GBP-Control**: обязательный управляющий поток.

## 3.  Модель протокола
GBP работает поверх QUIC [RFC9000], защищенного TLS 1.3 [RFC8446], и использует MLS [RFC9420] для группового управления ключами.

Узел:
- MUST поддерживать одно аутентифицированное QUIC-соединение на активную групповую сессию.
- MUST назначать Stream 0 как GBP-Control.
- MUST поддерживать значения StreamType 0..3.
- SHOULD изолировать flow-control и повторные передачи между классами потоков.

Исходный реестр StreamType:
- 0: control
- 1: audio
- 2: text
- 3: signal

## 4.  Состояние группы и членство
Каждый узел поддерживает:
- GroupID
- CurrentEpoch
- MemberSet
- ActiveStreams
- SecurityContext (ciphersuite, KDF, key schedule)
- CommitLog

Правила изменения состава:
- MUST выполняться только валидными MLS Commit.
- MUST монотонно увеличивать epoch.
- MUST отклонять неподтвержденные и устаревшие commit-сообщения.

## 5.  Криптографическая обработка
Полезная нагрузка GBP защищается MLS application traffic secrets, привязанными к epoch.

Узлы:
- MUST шифровать и аутентифицировать каждую полезную нагрузку ключами текущей epoch.
- MUST отклонять кадры с неизвестной, устаревшей или недопустимо будущей epoch.
- SHOULD выводить секреты подпротоколов через labeled HKDF контекст.

Рекомендуемые exporter-label:
- `gbp/control`
- `gbp/audio`
- `gbp/text`
- `gbp/signal`

## 6.  Формат кадра GBP

### 6.1.  Бинарная структура
```
GBPFrame {
  uint8    version;
  uint128  group_id;
  uint64   epoch;
  uint32   transition_id;
  uint8    stream_type;
  uint32   stream_id;
  uint16   flags;
  uint32   sequence_no;
  uint32   payload_size;
  bytes    encrypted_payload;
}
```

Биты `flags`:
- bit 0 (`0x0001`) O: упорядоченная доставка
- bit 1 (`0x0002`) R: надежная доставка
- bit 2 (`0x0004`) A: требуется подтверждение
- bit 3 (`0x0008`) S: системный кадр
- bit 4 (`0x0010`) C: критичное расширение

### 6.2.  Валидация
Получатель MUST проверять `version`, `group_id`, `epoch` и `payload_size` до маршрутизации полезной нагрузки. Некорректные кадры MUST отбрасываться без частичного применения.

## 7.  Семантика доставки
- При O: получатель MUST сохранять порядок в потоке.
- При R: отправитель MUST повторять до ACK/NACK или таймаута политики.
- При A: получатель MUST отправить ACK или NACK.
- Без R: узел MAY применять best-effort обработку.

## 8.  Ресинхронизация
При переподключении клиент:
1. MUST запросить `GroupStateDigest`.
2. MUST сравнить локальную и удаленную epoch.
3. MUST сравнить локальный и удаленный transition_id.
4. MUST выполнить MLS resync при расхождении.
5. SHOULD детерминированно открыть обязательные потоки.

Если синхронизация неуспешна, узел MUST перейти в неучаствующее состояние до явного rejoin.

## 9.  Обработка ошибок
Коды ошибок:
- `ERR_UNSUPPORTED_VERSION`
- `ERR_UNKNOWN_GROUP`
- `ERR_EPOCH_MISMATCH`
- `ERR_TRANSITION_MISMATCH`
- `ERR_REPLAY_DETECTED`
- `ERR_DECRYPT_FAILED`
- `ERR_COMMIT_INVALID`
- `ERR_STREAM_POLICY_VIOLATION`

Критические ошибки SHOULD завершать групповой транспортный контекст.

## 10.  Схемы сериализации

### 10.1.  CBOR (diagnostic)
```
{
  "v": uint,
  "gid": bstr,
  "ep": uint,
  "st": uint,
  "sid": uint,
  "fl": uint,
  "psz": uint,
  "pl": bstr
}
```

### 10.2.  Protobuf
```proto
syntax = "proto3";
package gbp;

message GBPFrame {
  uint32 version = 1;
  bytes group_id = 2;
  uint64 epoch = 3;
  uint32 stream_type = 4;
  uint32 stream_id = 5;
  uint32 flags = 6;
  uint32 payload_size = 7;
  bytes encrypted_payload = 8;
}
```

### 10.3.  FlatBuffers
```fbs
namespace gbp;

table GBPFrame {
  version:ubyte;
  group_id:[ubyte];
  epoch:ulong;
  stream_type:ubyte;
  stream_id:uint;
  flags:ushort;
  payload_size:uint;
  encrypted_payload:[ubyte];
}

root_type GBPFrame;
```

## 11.  IANA Considerations
Требуется реестр "GBP StreamType Registry" с начальными значениями из раздела 3.

## 12.  Security Considerations
Безопасность зависит от корректной сходимости MLS-состояния, аутентификации узлов и строгой проверки epoch. Реализации MUST удалять устаревший ключевой материал после смены epoch.

## 13.  References
### 13.1.  Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC8446] Rescorla, E., "The Transport Layer Security (TLS) Protocol Version 1.3".
- [RFC9000] Iyengar, J. and M. Thomson, "QUIC: A UDP-Based Multiplexed and Secure Transport".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
