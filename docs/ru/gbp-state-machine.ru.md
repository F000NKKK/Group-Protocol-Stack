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
Реализации MUST поддерживать `T_prepare_max`, `T_ready_max`, `T_execute_max`.

## 7. IANA Considerations
Новых действий IANA не требуется.

## 8. Security Considerations
Реализации MUST отклонять недопустимые переходы и не применять side effects до валидации состояния.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
