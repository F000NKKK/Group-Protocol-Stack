"""SFrame E2EE session and encryptor for GAP audio streams."""

from __future__ import annotations

import ctypes
from dataclasses import dataclass

from . import _native as N

#: AES-128-GCM ciphersuite (16-byte key).  Default.
AES_128_GCM: int = 0
#: AES-256-GCM ciphersuite (32-byte key).
AES_256_GCM: int = 1


@dataclass(frozen=True)
class SFrameDecryptResult:
    """Result of :meth:`SFrameSession.decrypt`."""

    plaintext: bytes
    """Decrypted Opus frame bytes."""

    sender_leaf: int
    """MLS leaf index of the sender."""


class SFrameSession:
    """SFrame E2EE session for one MLS epoch.

    Derives ``sframe_base_key`` from the MLS ``ExportSecret`` and provides
    :meth:`create_encryptor` and :meth:`decrypt` for the send and receive paths.

    Create a new session after every MLS commit (epoch change).

    Example::

        session = SFrameSession.create(mls, "gbp/sframe v1")
        enc = session.create_encryptor(mls, my_leaf_index)

        # Sender:
        payload = enc.encrypt(opus_frame)

        # Receiver:
        result = session.decrypt(payload)
        print(result.sender_leaf, result.plaintext)
    """

    __slots__ = ("_handle",)

    def __init__(self, handle: int) -> None:
        self._handle = handle

    @classmethod
    def create(
        cls,
        mls,
        label: str = "gbp/sframe v1",
        suite: int = AES_128_GCM,
    ) -> "SFrameSession":
        """Create a session from the current MLS context.

        :param mls:   :class:`~gbp_stack.MlsContext` at the current epoch.
        :param label: Export label (e.g. ``"gbp/sframe v1"``).
        :param suite: :data:`AES_128_GCM` (default) or :data:`AES_256_GCM`.
        :raises RuntimeError: If the native call fails.
        """
        label_bytes = label.encode("utf-8")
        handle = N.call_with_bytes(
            label_bytes,
            lambda ptr, length: N.gbp_sframe_session_create(
                mls._handle, suite, ptr, length
            ),
        )
        if not handle:
            raise RuntimeError(f"gbp_sframe_session_create: {N.last_error()}")
        return cls(handle)

    def create_encryptor(
        self,
        mls,
        leaf_index: int,
        label: str = "gbp/sframe v1",
        suite: int = AES_128_GCM,
    ) -> "SFrameEncryptor":
        """Create a per-sender encryptor for ``leaf_index``.

        :param mls:        Must be the same context used in :meth:`create`.
        :param leaf_index: The local sender's MLS leaf index.
        :param label:      Must match the label used in :meth:`create`.
        :param suite:      Must match the suite used in :meth:`create`.
        :raises RuntimeError: If the native call fails.
        """
        label_bytes = label.encode("utf-8")
        enc_handle = N.call_with_bytes(
            label_bytes,
            lambda ptr, length: N.gbp_sframe_encryptor_create(
                mls._handle, self._handle, leaf_index, suite, ptr, length
            ),
        )
        if not enc_handle:
            raise RuntimeError(f"gbp_sframe_encryptor_create: {N.last_error()}")
        return SFrameEncryptor(enc_handle)

    def decrypt(
        self,
        payload: bytes,
        extra_aad: bytes = b"",
    ) -> SFrameDecryptResult:
        """Decrypt one SFrame payload.

        :param payload:   Full SFrame payload (header + ciphertext + tag).
        :param extra_aad: Additional authenticated data used on the sender side.
        :raises RuntimeError: On decryption failure (wrong key, replay, etc.).
        """
        sender_leaf = ctypes.c_uint32(0)

        def _call(pay_ptr, pay_len):
            return N.call_with_bytes(
                extra_aad,
                lambda aad_ptr, aad_len: N.gbp_sframe_decrypt(
                    self._handle,
                    pay_ptr, pay_len,
                    aad_ptr, aad_len,
                    ctypes.byref(sender_leaf),
                ),
            )

        buf = N.call_with_bytes(payload, _call)
        plaintext = N.take_buffer(buf)
        if not plaintext and payload:
            raise RuntimeError(f"gbp_sframe_decrypt: {N.last_error()}")
        return SFrameDecryptResult(plaintext=plaintext, sender_leaf=sender_leaf.value)

    def close(self) -> None:
        """Free the native session handle."""
        if self._handle:
            N.gbp_sframe_session_free(self._handle)
            self._handle = 0

    def __enter__(self) -> "SFrameSession":
        return self

    def __exit__(self, *_) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()


class SFrameEncryptor:
    """Stateful per-sender SFrame encryptor.

    Maintains an internal counter that increments on every :meth:`encrypt` call.
    Obtain via :meth:`SFrameSession.create_encryptor`.
    """

    __slots__ = ("_handle",)

    def __init__(self, handle: int) -> None:
        self._handle = handle

    def encrypt(self, plaintext: bytes, extra_aad: bytes = b"") -> bytes:
        """Encrypt one audio frame.

        :param plaintext: Raw Opus frame bytes.
        :param extra_aad: Additional authenticated data; empty bytes if none.
        :returns: SFrame payload: ``sframe_header || ciphertext || GCM-tag``.
        :raises RuntimeError: On encryption failure.
        """

        def _call(pt_ptr, pt_len):
            return N.call_with_bytes(
                extra_aad,
                lambda aad_ptr, aad_len: N.gbp_sframe_encrypt(
                    self._handle, pt_ptr, pt_len, aad_ptr, aad_len
                ),
            )

        buf = N.call_with_bytes(plaintext, _call)
        result = N.take_buffer(buf)
        if not result and plaintext:
            raise RuntimeError(f"gbp_sframe_encrypt: {N.last_error()}")
        return result

    def close(self) -> None:
        """Free the native encryptor handle."""
        if self._handle:
            N.gbp_sframe_encryptor_free(self._handle)
            self._handle = 0

    def __enter__(self) -> "SFrameEncryptor":
        return self

    def __exit__(self, *_) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()
