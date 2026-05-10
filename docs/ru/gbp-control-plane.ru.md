# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Control Plane Messages

## Аннотация
Документ определяет сообщения GBP-Control, opcodes и процедуры перехода/восстановления.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.
Internet-Draft является рабочим документом IETF.

## 1. Введение
GBP-Control передается в StreamType 0.

## 2. Заголовок сообщения
```
GBPControl {
  uint16 opcode;
  uint32 request_id;
  uint32 sender_id;
  uint32 transition_id;
  uint32 args_length;
  bytes  args_cbor;
}
```

## 3. Реестр opcode (начальный)
- `0x0001 PREPARE_TRANSITION`
- `0x0002 READY_FOR_TRANSITION`
- `0x0003 EXECUTE_TRANSITION`
- `0x0004 ABORT_TRANSITION`
- `0x0005 GROUP_STATE_DIGEST_REQUEST`
- `0x0006 GROUP_STATE_DIGEST_RESPONSE`
- `0x0007 REPORT_INVALID_COMMIT`
- `0x0008 CAPABILITIES_ADVERTISE`
- `0x0009 ACK`
- `0x000A NACK`

## 4. Процедуры перехода
`PREPARE_TRANSITION -> READY_FOR_TRANSITION -> EXECUTE_TRANSITION` с детерминированным timeout fallback.

## 5. Процедуры восстановления
Поддерживаются `REPORT_INVALID_COMMIT` и digest-based resync.

## 6. IANA Considerations
Требуется реестр GBP Control Opcode.

## 7. Security Considerations
Управляющие сообщения MUST быть аутентифицированы и защищены от replay.

## 8. References
### 8.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
