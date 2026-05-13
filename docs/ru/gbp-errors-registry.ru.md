# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Error Code Registry

## Аннотация
Документ задает единую таксономию ошибок для GBP, GAP, GTP и GSP.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1. Соглашения
Ключевые слова BCP 14 применяются.

## 2. Объект ошибки
```
ErrorObject {
  uint16 code;
  uint8  class;
  bool   retryable;
  bool   fatal;
  string reason;
  bytes  details_cbor;
}
```

## 3. Классы ошибок
- `0x01` TRANSPORT
- `0x02` CRYPTO
- `0x03` STATE
- `0x04` POLICY
- `0x05` SCHEMA
- `0x06` AUTHZ

## 4. Диапазоны кодов
- `0x0000-0x0FFF` GBP
- `0x1000-0x1FFF` GAP
- `0x2000-0x2FFF` GTP
- `0x3000-0x3FFF` GSP
- `0xF000-0xFFFF` Private use

## 5. Начальные коды
Используются начальные наборы кодов из EN-версии документа. Полный список в `gbp-errors-registry.md`. Дополнительно к базовым `0x0001..0x0009` определены коды для control-plane таймаутов:

- `0x0009 ERR_TRANSITION_IN_PROGRESS` — Второй commit пришёл при активном pending transition
- `0x0010 ERR_PREPARE_TIMEOUT` — Координатор не дождался quorum READY
- `0x0011 ERR_READY_TIMEOUT` — Member не успел применить commit/welcome за `T_ready_max`
- `0x0012 ERR_EXECUTE_TIMEOUT` — Member не дождался EXECUTE_TRANSITION после READY
- `0x0013 ERR_COORDINATOR_GONE` — transport Координатора потерян, требуется handover
- `0x0014 ERR_DIGEST_MISMATCH` — несовпадение `member_set_root_hash` при Resync

## 6. Retryability/Fatality
Каждый код MUST иметь явно заданные признаки retryable/fatal. Матрица ниже нормативна для базовых GBP-кодов:

| Код | Retryable | Fatal | Причина |
|---|---|---|---|
| `0x0001 ERR_UNSUPPORTED_VERSION` | false | true | Невозможно восстановить без re-negotiation |
| `0x0002 ERR_UNKNOWN_GROUP` | false | true | Не та группа; сессия невалидна |
| `0x0003 ERR_EPOCH_MISMATCH` | true | false | Recover через Resync |
| `0x0004 ERR_TRANSITION_MISMATCH` | true | false | Recover через Resync |
| `0x0005 ERR_REPLAY_DETECTED` | false | false | Дроп фрейма |
| `0x0006 ERR_DECRYPT_FAILED` | true | false | Фрейм запечатан под другую MLS-эпоху (например PREPARE для свежего joiner'а) — нода MUST продолжать работу для следующего EXECUTE на общей эпохе. На повторных failures допустим Resync. |
| `0x0007 ERR_COMMIT_INVALID` | false | true | Stack-level integrity нарушен; abort transition + fresh KeyPackage |
| `0x0008 ERR_STREAM_POLICY_VIOLATION` | false | false | Дроп; deployment-policy решает escalation |
| `0x0009 ERR_TRANSITION_IN_PROGRESS` | false | false | Вызывающая сторона MUST сначала finalise или clear pending commit |
| `0x0010 ERR_PREPARE_TIMEOUT` | true | false | Координатор MAY переисустить PREPARE на следующем tid |
| `0x0011 ERR_READY_TIMEOUT` | true | false | Member возвращается в `T_IDLE` |
| `0x0012 ERR_EXECUTE_TIMEOUT` | true | false | Trigger Resync; участвовать в handover |
| `0x0013 ERR_COORDINATOR_GONE` | true | false | Members выбирают handover по §4.1 |
| `0x0014 ERR_DIGEST_MISMATCH` | false | true | Re-bootstrap как joiner |

## 7. IANA Considerations
Документ запрашивает реестр GBP Error Code.

## 8. Security Considerations
Ошибки MUST NOT раскрывать ключевой материал и чувствительные данные полезной нагрузки.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
