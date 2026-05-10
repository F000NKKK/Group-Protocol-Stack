# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP State Machine Specification

## Аннотация
Документ задает нормативные машины состояний для жизненного цикла узла GBP, переходов epoch и активации подпротоколов.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1. Введение
Документ является companion к `gbp_rfc.md`.

## 2. Соглашения
Ключевые слова BCP 14 применяются по [RFC2119]/[RFC8174].

## 3. Машина состояний узла
Состояния:
- `IDLE`
- `CONNECTING`
- `ESTABLISHING_GROUP`
- `ACTIVE`
- `RESYNCING`
- `FAILED`
- `CLOSED`

Переходы MUST соответствовать EN-версии документа.

## 4. Машина перехода epoch
Состояния:
- `T_IDLE`
- `T_PREPARED`
- `T_COMMIT_PROCESSED`
- `T_READY`
- `T_EXECUTED`
- `T_ABORTED`

Переход выполняется по `PREPARE_TRANSITION -> READY_FOR_TRANSITION -> EXECUTE_TRANSITION`.

## 5. Активация подпротоколов
Состояния:
- `DISABLED`
- `NEGOTIATING`
- `ENABLED`
- `DEGRADED`
- `SUSPENDED`

## 6. Таймауты
Реализации MUST поддерживать перечисленные таймеры. Default-значения нормативны для interop-развёртываний; deployment policy MAY сократить их, но MUST NOT увеличить без явной downgrade-договорённости.

| Таймер | Default | Владелец | Запускается при | Истекает когда |
|---|---|---|---|---|
| `T_prepare_max` | 5 с | Координатор | отправке `PREPARE_TRANSITION` | quorum READY ещё не достигнут |
| `T_ready_max` | 5 с | Member | приёме `PREPARE_TRANSITION` | локальная обработка commit/welcome не завершена |
| `T_execute_max` | 10 с | Member | отправке `READY_FOR_TRANSITION` | `EXECUTE_TRANSITION` не получен |
| `T_quorum_grace` | 2 с | Координатор | истечении `T_prepare_max` | дополнительный grace перед quorum-failure |
| `T_coordinator_grace` | 10 с | Member | молчании Координатора | разрешён coordinator-handover |

Истечение таймера MUST приводить к детерминированному fallback'у:
- **`T_prepare_max + T_quorum_grace` у Координатора**: отправить `ABORT_TRANSITION` с `reason_code = ERR_READY_TIMEOUT`. Координатор MAY переисустить PREPARE на следующей эпохе, исключив транспортно-недоступных.
- **`T_ready_max` у Member'а**: сбросить локальный pending (вернуться в `T_IDLE`). Member MUST NOT отправлять `READY_FOR_TRANSITION` ретроспективно. Если потом приходит `EXECUTE_TRANSITION` для tid, на который не было ready — переход в `RESYNCING` с запросом digest.
- **`T_execute_max` у Member'а**: предположить что Координатор упал. Триггер `RESYNCING`; при подтверждённом обрыве (transport closed) — участвовать в coordinator-handover по `gbp-control-plane.md` §4.1.

## 7. IANA Considerations
Новых действий IANA не требуется.

## 8. Security Considerations
Реализации MUST отклонять недопустимые переходы и не применять side effects до валидации состояния.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
