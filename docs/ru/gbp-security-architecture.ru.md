# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Informational                                   May 2026
# Expires: November 2026

# GBP Security Architecture

## Аннотация
Документ задает модель угроз и архитектуру безопасности стека GBP.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1. Введение
GBP использует MLS для групповых ключей и QUIC/TLS для транспорта.

## 2. Модель угроз
- внешние атакующие
- злонамеренные инсайдеры
- компрометация DS
- компрометация AS

## 3. Цели безопасности
- конфиденциальность
- целостность и подлинность
- FS
- PCS
- устойчивость к downgrade

## 4. Границы доверия
DS считается недоверенным для конфиденциальности/целостности полезной нагрузки.

## 5. Replay и ordering
Приложение MUST применять идентификаторы уникальности и replay-окна.

## 6. Downgrade resistance
Тихий downgrade MUST считаться нарушением политики.

## 7. Сценарии компрометации
Определены сценарии endpoint/DS/AS и требуемые mitigations.

## 8. IANA Considerations
Нет.

## 9. Security Considerations
Документ нормативно фиксирует допущения модели угроз для companion draft-ов.

## 10. References
### 10.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
