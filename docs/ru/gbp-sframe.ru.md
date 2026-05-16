# Рабочая группа по сетям                                          F000NK
# Интернет-черновик                             Команда Voluntas Progressus
# Предполагаемый статус: стандарт                               Май 2026
# Истекает: ноябрь 2026

# GBP SFrame Extension — E2EE для GAP аудиопотоков

## Аннотация

Этот документ задаёт схему деривации ключей SFrame и кодирования заголовков,
используемых стеком Group Protocol Stack (GBP) для обеспечения E2EE
медиапотоков GAP (Group Audio Protocol).  Схема выводит per-sender AES-GCM
ключи из MLS ExportSecret, кодирует идентификатор отправителя в компактном
заголовке SFrame и поддерживает per-sender скользящее окно защиты от replay
на 1024 элемента.

## 1. Введение

GAP sub-protocol (StreamType 1) доставляет аудиофреймы Opus через SFU,
который выполняет selective forwarding, pacing пакетов и обработку NACK.
SFU должен видеть RTP/transport заголовки, но НЕ ДОЛЖЕН иметь доступ к
медиа payload.

SFrame [draft-ietf-sframe-enc] решает эту задачу, шифруя только медиапоток,
оставляя транспортные заголовки в открытом виде.

## 2. Положение в стеке

```
┌──────────────────────────────────────────────────┐
│         Транспортное шифрование (SRTP / DTLS)    │  ← клиент ↔ SFU
│  ┌────────────────────────────────────────────┐  │
│  │      SFrame (данная спецификация)          │  │  ← E2E клиент ↔ клиент
│  │   ┌──────────────────────────────────────┐ │  │
│  │   │  Медиафрейм (Opus / VP8)             │ │  │
│  │   └──────────────────────────────────────┘ │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

## 3. Схема ключей

### 3.1 Деривация базового ключа

После каждого MLS коммита каждый участник ДОЛЖЕН вывести:

```
sframe_base_key = MLS.ExportSecret(
    label   = <метка приложения>,      ; напр. "gbp/sframe v1"
    context = epoch.to_be_bytes(),     ; 8 байт big-endian
    length  = 32
)
```

### 3.2 Деривация per-sender ключей

Для каждого участника с MLS leaf index `i`:

```
participant_key_i = HKDF-Expand(
    PRK  = sframe_base_key,
    info = "gbp sframe key " || leaf_index_i.to_be_bytes(),  ; 4 байта BE
    L    = L_key
)

participant_salt_i = HKDF-Expand(
    PRK  = sframe_base_key,
    info = "gbp sframe salt " || leaf_index_i.to_be_bytes(), ; 4 байта BE
    L    = 12
)
```

`L_key`: AES-128-GCM → 16, AES-256-GCM → 32.
Хеш-функция для HKDF: SHA-256.

### 3.3 Нонс фрейма

```
nonce = participant_salt_i XOR (CTR.to_le_bytes() || 0x00_00_00_00)
```

## 4. Заголовок SFrame

```
 0                   1
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- - -
|V|  K (3)   |   C (4)   | KID...CTR...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- - -
```

* `V` (1 бит) — версия, ДОЛЖЕН быть `0`.
* `K` (3 бита) — длина KID в байтах минус один.
* `C` (4 бита) — длина CTR в байтах минус один.

### 4.1 Кодирование KID

```
KID = (epoch << 16) | (leaf_index & 0xFFFF)
```

### 4.2 Структура SFrame payload

```
SFrame payload = SFrame_header || AEAD_ciphertext || AEAD_tag
```

## 5. AEAD шифрование

```
ciphertext, tag = AEAD.Seal(
    key   = participant_key_i,
    nonce = nonce,
    plain = encoded_media_frame,
    aad   = SFrame_header || extra_aad
)
```

## 6. Ротация ключей

`sframe_base_key` ДОЛЖЕН быть перевыведен после каждого MLS commit.
Реализации ДОЛЖНЫ создавать новый `SFrameSession` после каждого успешного
`EXECUTE_TRANSITION` и ДОЛЖНЫ сбрасывать encryptors/decryptors предыдущей эпохи.

## 7. Защита от replay

Реализации ДОЛЖНЫ поддерживать скользящее окно на 1024 записи per `(KID, sender)`.

* Счётчики, отстающие более чем на 1024 позиции, ДОЛЖНЫ отклоняться.
* Дубликаты счётчиков ДОЛЖНЫ отклоняться до дешифрования.
* Окно ДОЛЖНО сбрасываться при смене эпохи.

## 8. Набор шифров

| Набор           | Длина ключа | AEAD        |
|-----------------|-------------|-------------|
| AES-128-GCM     | 16 байт     | AES-128-GCM |
| AES-256-GCM     | 32 байта    | AES-256-GCM |

По умолчанию: AES-128-GCM.

## 9. Заметки по реализации

Rust-крейт `gbp-sframe` реализует данную спецификацию.
Семейство FFI `gbp_sframe_*` в `gbp-stack-ffi` предоставляет API для
потребителей .NET, Node.js и Python.

## 10. Соображения безопасности

- Повторное использование ключа: конструкция нонса исключает повтор для
  одного `(epoch, leaf_index, CTR)`.
- Прямая секретность: ротация базового ключа на каждый коммит обеспечивает FS.
- Replay: окно на 1024 записи защищает от replay при неупорядоченной доставке.

## 11. Ссылки

- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
- [draft-ietf-sframe-enc] Jennings, C., et al., "Secure Frame (SFrame)".
- `gap_rfc.ru.md` — спецификация GAP.
- `gbp-mls-binding.ru.md` — привязка GBP/MLS.
