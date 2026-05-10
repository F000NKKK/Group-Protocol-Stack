# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP — процедура выхода участника

## Аннотация
Документ задаёт нормативную процедуру удаления участника из GBP-группы — как для добровольного leave (по инициативе самого участника через GSP), так и для принудительного (transport-обрыв, policy-enforcement, действие модератора). Дополняет `gbp_rfc.ru.md`, `gbp-control-plane.ru.md` и `gbp-mls-binding.ru.md`.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.

## 1. Соглашения
BCP 14 применяется.

## 2. Область применения
Процедура leave ротирует MLS-эпоху, чтобы traffic-secrets ушедшего участника были невалидны для нового application-трафика. Forward-secrecy после executed-transition гарантируется самим MLS Commit (RFC 9420 §12.3). Процедура НЕ восстанавливает сообщения, зашифрованные до перехода — они остаются дешифруемыми любым, кто хранил ratchet-state старой эпохи, в том числе ушедшим.

## 3. Триггеры
Координатор MUST инициировать leave-переход в одном из случаев:

1. **Добровольный leave** — участник отправляет `GSP { signal_type = LEAVE (101) }` (см. `gsp_rfc.ru.md`). Координатор валидирует authorization по GSP role-matrix.
2. **Принудительный disconnect** — DS уведомляет Координатора о закрытии транспорта Active-участника, и молчание держится дольше `T_coordinator_grace`. Механизм DS-уведомлений deployment-зависим; см. `gbp-mls-binding.ru.md` §5.
3. **Удаление модератором** — участник с ролью `moderator` отправляет `GSP { signal_type = LEAVE, target = X }` против другого члена.
4. **Policy-enforcement** — повторные fatal-нарушения от участника превышают deployment-thresholds.

Если уходит сам Координатор — сначала MUST завершиться coordinator-handover (§4.1 control-plane), затем новый Координатор инициирует leave.

## 4. Процедура

```
Шаг  Актор          Действие
---  -----          --------
 1   Координатор    Валидация триггера, определение target leaf_index в MLS-дереве.
 2   Координатор    mls.remove_members([leaf_index]) -> commit_bytes
                    MLS-state Координатора локально продвигается на новую эпоху.
 3   Координатор    next_tid = last_transition_id + 1.
 4   Координатор    Broadcast PREPARE_TRANSITION всем оставшимся Active-членам
                    (target = 0), args = { commit: commit_bytes, removed: target }.
                    Уходящий участник в рассылке НЕ участвует.
 5   Каждый         Применить commit через mls.process_message(commit_bytes);
     оставшийся     MLS-state продвигается. Отправить READY_FOR_TRANSITION
     member         (target = coordinator_id).
 6   Координатор    На quorum READY в окне T_ready_max + T_quorum_grace:
                    broadcast EXECUTE_TRANSITION (target = 0).
                    На timeout: broadcast ABORT_TRANSITION; retry с новым tid.
 7   Каждый         apply_transition(next_tid) -> current_epoch++,
     оставшийся     last_transition_id = next_tid, replay window очищен.
     member
 8   Координатор    То же что шаг 7 локально.
 9   Ушедший        MAY наблюдать PREPARE_TRANSITION через DS. MUST NOT
     участник       пытаться участвовать. Его MLS-state не продвигается.
                    Его application-frames после шага 8 будут отвергнуты
                    оставшимися с ERR_DECRYPT_FAILED.
```

## 5. Конкурентные leave
Если два leave-триггера срабатывают конкурентно (например, модератор удаляет A, а B шлёт voluntary LEAVE), Координатор MUST:

1. Поставить оба в pending-queue.
2. Выпустить как два отдельных перехода ИЛИ забатчить в один MLS commit с двумя Remove-предложениями. При батчинге PREPARE_TRANSITION несёт `removed: [A, B]` в args.
3. NEVER не сворачивать leave в in-flight add-переход; add MUST завершиться или абортнуться первым.

## 6. Crash и re-bootstrap
Если member упал посреди leave-перехода (отправил READY, но не получил EXECUTE), recovery идёт по `gbp-control-plane.ru.md` §5.2 (Resync). Запросить `GROUP_STATE_DIGEST`, переиграть пропущенные `EXECUTE_TRANSITION`, вернуться в Active.

Если Координатор упал между шагами 4 и 6, новый Координатор (handover) MUST трактовать in-flight leave как ABORTED, переисчислить commit и выпустить fresh PREPARE на следующем tid.

## 7. Восстановление ушедшего участника
Ушедший, желающий вернуться, MUST:
1. Сгенерировать fresh KeyPackage (NEVER не переиспользовать прошлый).
2. Опубликовать его через стандартный add-flow.
3. Получить новый MemberID; прежний MemberID retired безвозвратно (§2 `gbp_rfc.ru.md`).

## 8. Security Considerations
- **Forward-secrecy**: после EXECUTE_TRANSITION traffic-secrets новой эпохи деривированы из коммита, исключающего leaf ушедшего. Ушедший не может дешифровать новый трафик.
- **Past traffic**: всё, зашифрованное до перехода, остаётся дешифруемым любым, кто хранил ключи старой эпохи, в т.ч. ушедшим. Приложения, требующие deniability или PCS против past traffic, MUST ротировать ключи проактивно, не полагаясь на leave-on-departure.
- **Ghost members**: Координатор MUST NOT включать в PREPARE участника, чей transport закрыт, но который ещё не формально удалён — иначе quorum может зависнуть бессрочно.
- **Replay**: освобождённый MemberID MUST NOT быть переиспользован (§2 `gbp_rfc.ru.md`). Future-joiner с тем же MemberID создал бы неоднозначность в replay-window state по истории.

## 9. References
- [RFC2119], [RFC8174], [RFC9420]
- `gbp_rfc.ru.md`, `gbp-control-plane.ru.md`, `gbp-mls-binding.ru.md`, `gsp_rfc.ru.md`
