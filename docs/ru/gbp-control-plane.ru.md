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

### 4.1 Роль Координатора
В каждый момент времени ровно один Active-член группы для данного GroupID выступает **Координатором**. Только Координатор имеет право выпускать `PREPARE_TRANSITION`, `EXECUTE_TRANSITION` и `ABORT_TRANSITION`.

Правила выбора:
- Создатель группы становится начальным Координатором сразу после `bootstrap_creator`.
- Если Координатор недоступен (transport disconnect, fatal error, добровольный leave), роль переходит к **участнику с минимальным MemberID** среди оставшихся Active. Новый Координатор MUST разослать `CAPABILITIES_ADVERTISE` с флагом `coordinator_claim=true` в args.
- Member MUST принять coordinator-claim только после уведомления `MemberLeft` от прежнего Координатора либо после `T_coordinator_grace = 2 * T_ready_max` молчания.
- Конфликт двух одновременных claim'ов разрешается по минимальному MemberID; проигравший MUST само-демотироваться.

Не-Координатор MUST молча игнорировать любые `PREPARE_TRANSITION` / `EXECUTE_TRANSITION` / `ABORT_TRANSITION` ошибочно отправленные им самим (defense in depth) и MUST залогировать ошибку.

### 4.2 Инвариант одного pending-перехода
Координатор MUST NOT держать более одного pending-перехода одновременно. Конкурентные `add` / `remove` запросы MUST помещаться в очередь и сериализоваться в один `PREPARE_TRANSITION` на каждый `transition_id`. Несколько MLS-предложений MAY быть забатчены в один commit, но MUST разделять одно TransitionID.

### 4.3 Порядок Welcome / PREPARE при добавлении
При приёме нового члена Координатор:
1. Вычисляет MLS commit + Welcome (`mls.invite_full`) — обе части возвращаются.
2. Рассылает `PREPARE_TRANSITION` (target=0) существующим членам с новым `transition_id` и **MLS Commit** в `args.commit`.
3. Параллельно unicast'ом отправляет **MLS Welcome** новому члену (target = future_member_id).
4. Новый член считается «кандидатом» в течение этого перехода и попадает в quorum READY только после `accept_welcome` и собственного `READY_FOR_TRANSITION`.

Получатель MUST применить Commit ДО отправки `READY_FOR_TRANSITION`. При отсутствии / повреждении / отказе MLS — `NACK { code = ERR_COMMIT_INVALID }` и блокировка перехода.

### 4.4 Prepare
Координатор шлёт `PREPARE_TRANSITION` с новым `transition_id`, новым `epoch` (после commit) и MLS Commit в `args.commit`. Получатели создают локальный pending-контекст, валидируют version/group_id, применяют MLS Commit, переходят `T_IDLE -> T_PREPARED -> T_COMMIT_PROCESSED`.

### 4.5 Ready
Member отправляет `READY_FOR_TRANSITION` только когда MLS-эпоха продвинулась (Commit применён или Welcome принят) и все локальные предусловия выполнены. Sender MUST использовать тот же `transition_id`, который пришёл в PREPARE. Member движется `T_COMMIT_PROCESSED -> T_READY`.

### 4.6 Execute
Координатор шлёт `EXECUTE_TRANSITION` когда quorum READY достигнут или сработал timeout. По умолчанию quorum = **все Active-члены**, включая нового кандидата (если есть), в течение `T_ready_max`. Если хоть один Active не подтвердил в окне `T_ready_max + T_quorum_grace`, Координатор MUST разослать `ABORT_TRANSITION` и переисустить PREPARE на следующей эпохе, исключив молчащего, если transport объявил его недоступным.

Получатели атомарно вызывают `node.apply_transition(tid)`: `current_epoch++`, `last_transition_id = tid`, replay window очищается, `T_READY -> T_EXECUTED`.

### 4.7 Abort
Координатор или policy-engine шлёт `ABORT_TRANSITION` с `args.reason_code`. Получатели сбрасывают pending-state, откатывают локально-применённый MLS commit (или восстанавливаются через Resync), возвращаются в `T_IDLE`.

## 4b. Per-opcode TransitionID валидация
Получатели MUST валидировать `c.transition_id` против локального FSM-state по таблице ниже. Фреймы, не прошедшие валидацию, MUST отвергаться с `ERR_TRANSITION_MISMATCH` (`0x0004`) и MUST NOT продвигать state.

| Opcode | Условие |
|---|---|
| `0x0001 PREPARE_TRANSITION` | `c.tid > last_tid` AND (`pending_tid == 0` OR `pending_tid == c.tid`). Re-issue для уже-pending tid допустим (идемпотентность). |
| `0x0002 READY_FOR_TRANSITION` | `pending_tid != 0` AND `c.tid == pending_tid` |
| `0x0003 EXECUTE_TRANSITION` | `pending_tid != 0` AND `c.tid == pending_tid` |
| `0x0004 ABORT_TRANSITION` | `pending_tid != 0` AND `c.tid == pending_tid` |
| `0x0005 GROUP_STATE_DIGEST_REQUEST` | informational; без ограничений |
| `0x0006 GROUP_STATE_DIGEST_RESPONSE` | informational; без ограничений |
| `0x0007 REPORT_INVALID_COMMIT` | informational; без ограничений |
| `0x0008 CAPABILITIES_ADVERTISE` | informational; без ограничений |
| `0x0009 ACK` / `0x000A NACK` | echo `request_id`; tid информационный |

CRITICAL flag (`0x0010`) MUST NOT применяться к control-stream фреймам в §6.2-проверке: tid control-stream'а валидируется по этой таблице. Application-stream фреймы сохраняют CRITICAL-проверку из `gbp_rfc.ru.md` §6.2.

Sender-side state mirroring: при отправке `PREPARE_TRANSITION` Координатор MUST локально установить `pending_transition_id = c.tid` и `transition_state = T_PREPARED`. Аналогично на `ABORT_TRANSITION` — очистить `pending_transition_id`.

## 4c. Схемы аргументов control-сообщений (informative)
Поле `args` — opcode-зависимый blob. Reference-имплементация выдаёт его в событии как `args_b64`:

- **`PREPARE_TRANSITION`**: raw TLS-serialised MLS Commit. Получатели передают в `mls.process_message(args)`.
- **`READY_FOR_TRANSITION`**: пусто.
- **`EXECUTE_TRANSITION`**: пусто (локальный `apply_transition(tid)` достаточен).
- **`ABORT_TRANSITION`**: опциональный CBOR `{ "reason_code": uint }`.
- **`GROUP_STATE_DIGEST_REQUEST` / `GROUP_STATE_DIGEST_RESPONSE`**: CBOR-карты по §5.3.
- **`REPORT_INVALID_COMMIT`**: CBOR-карта по §5.1.
- **`CAPABILITIES_ADVERTISE`**: CBOR `{ "version": uint, "features": [tstr], ? "coordinator_claim": bool }`.
- **`ACK` / `NACK`**: CBOR-карта с echo `request_id`; NACK дополнительно несёт `ErrorObject`.

## 5. Процедуры восстановления

### 5.1 Восстановление при невалидном Commit
Получатель, обнаруживший некорректный Commit, MUST отправить `REPORT_INVALID_COMMIT`. Args (CBOR map):

```
ReportInvalidCommitArgs = {
  "transition_id": uint,         ; обязательно
  "reason_code":   uint,         ; обязательно; код из gbp-errors-registry
  "commit_hash":   bstr / nil,   ; опционально; SHA-256 байт коммита
  ? "details":     tstr          ; опционально; без секретов
}
```

После отправки репортер MUST очистить pending-state, запросить fresh KeyPackage workflow (если был joiner), либо запустить Resync (§5.2). Application data MUST быть приостановлены до следующего `EXECUTE_TRANSITION`.

Координатор на приёме `REPORT_INVALID_COMMIT`:
- MUST разослать `ABORT_TRANSITION` с `reason_code = ERR_COMMIT_INVALID`.
- MUST NOT повторять тот же commit байт-в-байт.
- SHOULD пере-вычислить commit (с новым ratchet-step) и выпустить новый PREPARE.

### 5.2 Resync
Клиент с расходящимся state'ом (frame отвергнут с `ERR_EPOCH_MISMATCH` или `ERR_TRANSITION_MISMATCH`) MUST перейти в `RESYNCING` и отправить `GROUP_STATE_DIGEST_REQUEST` Координатору (или любому Active-члену, если Координатор неизвестен).

### 5.3 Формат GROUP_STATE_DIGEST
Args для `GROUP_STATE_DIGEST_REQUEST` (0x0005):

```
GroupStateDigestRequest = {
  "since_tid": uint,
  ? "since_epoch": uint
}
```

Args для `GROUP_STATE_DIGEST_RESPONSE` (0x0006):

```
GroupStateDigestResponse = {
  "epoch":                uint,
  "last_transition_id":   uint,
  "member_set_root_hash": bstr,           ; SHA-256 над канонической CBOR-сериализацией
                                          ;   отсортированного массива MemberID
  "control_log_tail":     [ ControlLogEntry ], ; до 64 записей с `since_tid`
  ? "coordinator_id":     uint
}

ControlLogEntry = {
  "transition_id": uint,
  "opcode":        uint,
  "sender_id":     uint,
  "args_digest":   bstr     ; SHA-256 от args_cbor исходного control msg
}
```

Если `since_tid` старее минимально-сохранённой записи у responder'а (default retention: 64 transitions), responder SHOULD отправить полный digest с флагом `truncated=true`; полное восстановление — через rejoin (фреш KeyPackage).

После обработки responder'а requester MUST проверить совпадение `member_set_root_hash` со своим MLS-видом, последовательно применить пропущенные `EXECUTE_TRANSITION`-ы, перейти `RESYNCING -> ACTIVE`. При hash-mismatch — `REPORT_INVALID_COMMIT` и трактовка сессии как фатально расходящейся (re-bootstrap как joiner).

## 6. IANA Considerations
Требуется реестр GBP Control Opcode.

## 7. Security Considerations
Управляющие сообщения MUST быть аутентифицированы и защищены от replay.

## 8. References
### 8.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
