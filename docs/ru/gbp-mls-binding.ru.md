# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP / MLS — связка

## Аннотация
Документ определяет контракт между control-plane GBP и нижележащим MLS-state-machine (RFC 9420). Уточняет какие MLS-сообщения видны на каком уровне GBP, кто отвечает за их распространение и как MLS-эпохи и proposal-типы соотносятся с GBP TransitionID.

## Статус документа
Этот Internet-Draft представлен в полном соответствии с BCP 78 и BCP 79.

## 1. Соглашения
BCP 14 применяется.

## 2. Видимость MLS-сообщений
RFC 9420 определяет два релевантных типа сообщений для смены состава:
- **Welcome** — отправляется *новым* членам; несёт state, нужный для bootstrap MLS-группы у joiner'а.
- **Commit** — отправляется *существующим* членам; инструктирует применить набор предложений (Add/Update/Remove) и продвинуть epoch.

GBP REQUIRES различать пути доставки:
- Welcome MUST быть **unicast** новому члену.
- Commit MUST быть **broadcast** всем существующим, встроенный в `args.commit` сообщения `PREPARE_TRANSITION`.

Реализация-баг, рассылающая только Welcome, оставляет существующих членов без возможности продвинуть MLS-эпоху и ломает весь последующий application-трафик. Реализации MUST экспонировать оба сообщения через MLS API.

## 3. Обязательная поверхность MLS API
GBP MLS-обёртка MUST предоставлять:

```
mls.invite(key_packages: [KeyPackage]) -> { commit: bytes, welcome: bytes }
mls.remove_members(leaf_indices: [u32]) -> { commit: bytes }
mls.process_message(message: bytes) -> ProcessedMessageKind
mls.accept_welcome(welcome: bytes) -> ()
mls.epoch() -> u64
mls.group_id() -> [u8; 16]
mls.export_key_package() -> bytes
```

`process_message` MUST уметь обрабатывать Commit-сообщения и REQUIRED каждому существующему члену. `ProcessedMessageKind` различает Commit / Application / Proposal — для control-plane GBP актуален только Commit.

`invite` и `remove_members` MUST продвигать локальный MLS-state немедленно (через `merge_pending_commit`), чтобы видение Координатора совпадало с post-transition state'ом, использованным для деривации PREPARE-байт.

## 4. Соответствие MLS Epoch и GBP TransitionID
- Каждый принятый MLS Commit увеличивает `mls.epoch` на 1.
- Каждый `EXECUTE_TRANSITION` несёт тот же `transition_id`, что был объявлен в соответствующем `PREPARE_TRANSITION`.
- Реализация MUST поддерживать инвариант `node.current_epoch == mls.epoch()` в любом стационарном состоянии (post-EXECUTE, pre-next-PREPARE).
- Во время перехода: `mls.epoch()` продвигается при обработке Commit (шаг 5 add/leave); `node.current_epoch` продвигается на `apply_transition` (шаг 7). Между этими двумя точками узел в `T_READY` и MUST NOT отправлять application data.

## 5. Обязанности DS
Реализация Delivery Service для GBP MUST:
1. Форвардить `PREPARE_TRANSITION` (target=0) всем Active-членам кроме исходного отправителя.
2. Форвардить Welcome unicast'ы, адресованные конкретному MemberID (target=N).
3. Детектировать transport-закрытия и эмитить уведомление `MemberLeft { member_id, reason }` Координатору в окне `T_coordinator_grace`.
4. Предоставлять per-DS монотонную последовательность на форвардных control-сообщениях для tie-break ordering из `gbp_rfc.ru.md` §7a.

P2P fallback (без DS) MUST симулировать пп. 1-3 в процессе Координатора; п. 4 редуцируется до локального accept-order.

## 6. Bootstrap состояния joiner'а
Joiner, получивший Welcome, MUST:
1. `mls.accept_welcome(welcome_bytes)` — поднять MLS-группу на post-commit-эпохе.
2. Прочитать `mls.epoch()` и `mls.group_id()` из получившегося state.
3. Создать GBP-узел через `gbp_node_create(member_id, group_id_16)`.
4. Вызвать `gbp_node_bootstrap_joiner(epoch=0)` — `current_epoch` GBP-узла стартует с 0 и продвигается ТОЛЬКО через предстоящий `EXECUTE_TRANSITION`. MLS epoch из шага 2 на этом моменте информационная; она будет согласована после того как joiner отправит READY и получит EXECUTE для того же TransitionID, по которому его принимали.
5. Дождаться `PREPARE_TRANSITION`, несущего commit того же приёма. Координатор MUST отправить его, даже если joiner уже имеет post-commit state из Welcome — GBP-уровню joiner'а нужна явная transition-запись, чтобы продвинуть `current_epoch` и `last_transition_id`.

## 7. State Координатора после invite
Координатор, вызвавший `mls.invite_full`:
1. Имеет `mls.epoch()` уже продвинутую (через `merge_pending_commit`).
2. MUST NOT отправлять application data — `node.current_epoch` ещё старый.
3. MUST отправить `PREPARE_TRANSITION` с новым `transition_id` и встроить commit-bytes.
4. MUST NOT вызывать `apply_transition` локально до broadcast'а `EXECUTE_TRANSITION`. Координатор проходит ту же последовательность `T_PREPARED -> T_COMMIT_PROCESSED -> T_READY -> T_EXECUTED` что и любой другой member, неявно учитываясь в quorum READY.

## 8. Out-of-order Welcome и Commit
DS не гарантирует, что Welcome joiner'а придёт раньше PREPARE+Commit существующим, либо наоборот. Оба порядка валидны:
- Existing-member получает PREPARE до того, как joiner принял Welcome — quorum-count ждёт READY от joiner'а (потенциально до `T_ready_max`).
- Joiner принял Welcome до того, как existing-members обработали Commit — joiner ждёт в `T_PREPARED` пока через PREPARE не придёт `args.commit`; если PREPARE пришёл первым и commit извлечён — joiner уже в `T_COMMIT_PROCESSED`.

Реализация MUST быть устойчивой к обоим порядкам.

## 9. Security Considerations
- MLS-state Координатора продвигается eagerly (шаг 1 §7). Если переход aborts, Координатор MUST уметь откатиться. RFC 9420 §12 поддерживает это только если `merge_pending_commit` ещё не вызвался. Реализация SHOULD откладывать merge до получения quorum READY; если обёртка merge'ит eagerly, abort требует re-bootstrap MLS-контекста Координатора (приемлемо в deployments, где abort редок, но MUST документироваться).
- Welcome-сообщения MUST передаваться по конфиденциальному транспорту. Утечка Welcome любому, кроме intended-joiner, позволяет ему реконструировать секреты новой эпохи.
- Атака replay устаревших PREPARE+Commit MUST детектироваться через монотонность TransitionID (`gbp_rfc.ru.md` §7a).

## 10. References
- [RFC2119], [RFC8174], [RFC9420]
- `gbp_rfc.ru.md`, `gbp-control-plane.ru.md`, `gbp-state-machine.ru.md`, `gbp-leave-flow.ru.md`
