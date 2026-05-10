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
Используются начальные наборы кодов из EN-версии документа.

## 6. Retryability/Fatality
Каждый код MUST иметь явно заданные признаки retryable/fatal.

## 7. IANA Considerations
Документ запрашивает реестр GBP Error Code.

## 8. Security Considerations
Ошибки MUST NOT раскрывать ключевой материал и чувствительные данные полезной нагрузки.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
