# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Interoperability Profile

## Аннотация
Документ определяет классы соответствия и требования интероперабельности реализаций GBP.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1. Классы соответствия
- Class A: GBP + GSP
- Class B: Class A + GTP
- Class C: Class B + GAP

## 2. Обязательные возможности
- QUIC + TLS 1.3
- обработка MLS epoch
- процедуры GBP-Control
- поддержка Error registry
- replay window enforcement

## 3. Переговоры версий
При отсутствии пересечения версий handshake MUST завершаться ошибкой.

## 4. Interop-тесты
1. Создание группы.
2. Add/remove member transition.
3. Tie-break конкурентных commit.
4. Восстановление при invalid commit.
5. Replay rejection.
6. GAP overlap-key decryption.
7. GTP idempotent duplicates.
8. GSP authorization NACK.

## 5. IANA Considerations
Новых действий IANA не требуется.

## 6. Security Considerations
Interop-послабления MUST NOT ослаблять требования MLS, replay и downgrade resistance.

## 7. References
### 7.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
