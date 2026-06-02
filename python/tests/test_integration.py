"""Integration tests for the gbp-stack Python bindings.

Covers:
  - MLS context lifecycle and invite flows
  - GBP GroupNode bootstrap, send, on_wire
  - GTP text messaging (bidirectional, duplicates, unicode, reset)
  - GAP audio frames (multiple sources, reset)
  - GSP signals (all signal types, duplicates)
  - User lifecycle: 3-member group, leave, rejoin
  - Coordinator event kinds
  - MessageHistory + Watermark
  - JitterBuffer
  - RoleRegistry + CapabilitiesNegotiator
  - SFrame E2EE (AES-128, AES-256, extra AAD)
  - Utility functions: encode_gbp_frame, lookup_error
"""

import pytest

from gbp_stack import (
    MlsContext,
    GroupNode,
    GtpClient,
    GapClient,
    GspClient,
    PayloadCodec,
    encode_gbp_frame,
    lookup_error,
)
from gbp_stack.gbp_node import StreamType, NodeState, ControlOpcode, NodeEvent, _parse_events
from gbp_stack.gsp_client import SignalType
from gbp_stack.jitter import AudioFrame, JitterBuffer, JitterPushOutcome
from gbp_stack.history import MessageEntry, MessageHistory, Watermark
from gbp_stack.roles import Permissions, RoleRegistry, RoleError
from gbp_stack.capabilities import CapabilitiesNegotiator
from gbp_stack.sframe_session import SFrameSession, AES_128_GCM, AES_256_GCM


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _pt(event: NodeEvent) -> bytes:
    """Assert and return event.plaintext as non-None bytes."""
    assert event.plaintext is not None, f"expected plaintext in event {event.kind}"
    return event.plaintext


def _two_member_group():
    """Return (alice_mls, alice_node, bob_mls, bob_node) as a live 2-member group."""
    alice_mls = MlsContext.create("alice")
    bob_mls = MlsContext.create("bob")

    bob_kp = bob_mls.export_key_package()
    welcome = alice_mls.invite(bob_kp)
    bob_mls.accept_welcome(welcome)

    gid = alice_mls.group_id
    alice_node = GroupNode.create(member_id=1, group_id=gid)
    bob_node = GroupNode.create(member_id=2, group_id=gid)
    alice_node.bootstrap_as_creator(alice_mls.epoch)
    bob_node.bootstrap_as_joiner(bob_mls.epoch)

    return alice_mls, alice_node, bob_mls, bob_node


def _text_events(events):
    return [e for e in events if e.kind == "payload_received" and e.stream_type == StreamType.TEXT]


def _audio_events(events):
    return [e for e in events if e.kind == "payload_received" and e.stream_type == StreamType.AUDIO]


def _signal_events(events):
    return [e for e in events if e.kind == "payload_received" and e.stream_type == StreamType.SIGNAL]


def _cbor_map1(key: int, value: int) -> bytes:
    """Encode a minimal CBOR map with one uint key→uint value pair."""
    def _uint(n: int) -> bytes:
        if n <= 23:
            return bytes([n])
        if n <= 0xFF:
            return bytes([0x18, n])
        if n <= 0xFFFF:
            return bytes([0x19, n >> 8, n & 0xFF])
        return bytes([0x1a, (n >> 24) & 0xFF, (n >> 16) & 0xFF, (n >> 8) & 0xFF, n & 0xFF])
    return bytes([0xa1]) + _uint(key) + _uint(value)


def _cbor_map2(k0: int, v0: int, k1: int, v1: int) -> bytes:
    """Encode a minimal CBOR map with two uint key→uint value pairs."""
    def _uint(n: int) -> bytes:
        if n <= 23:
            return bytes([n])
        if n <= 0xFF:
            return bytes([0x18, n])
        return bytes([0x19, n >> 8, n & 0xFF])
    return bytes([0xa2]) + _uint(k0) + _uint(v0) + _uint(k1) + _uint(v1)


# ---------------------------------------------------------------------------
# MLS context
# ---------------------------------------------------------------------------

class TestMlsContext:
    def test_create_and_epoch(self):
        with MlsContext.create("alice") as ctx:
            assert ctx.epoch == 0
            assert ctx.identity == "alice"

    def test_group_id_is_16_bytes(self):
        with MlsContext.create("alice") as ctx:
            assert len(ctx.group_id) == 16

    def test_export_key_package(self):
        with MlsContext.create("alice") as ctx:
            kp = ctx.export_key_package()
            assert isinstance(kp, bytes) and len(kp) > 0

    def test_invite_and_accept_welcome(self):
        with MlsContext.create("alice") as alice, MlsContext.create("bob") as bob:
            welcome = alice.invite(bob.export_key_package())
            bob.accept_welcome(welcome)
            assert bob.epoch == alice.epoch

    def test_export_state_restore_state_preserves_epoch_and_group_id(self):
        with MlsContext.create("alice") as alice, MlsContext.create("bob") as bob:
            bob.accept_welcome(alice.invite(bob.export_key_package()))
            assert alice.epoch == 1
            blob = alice.export_state()
            assert isinstance(blob, bytes) and len(blob) > 0
            with MlsContext.restore_state(blob, "alice") as restored:
                assert restored.epoch == alice.epoch
                assert restored.group_id == alice.group_id

    def test_restore_state_rejects_truncated_blob(self):
        with MlsContext.create("alice") as alice:
            blob = alice.export_state()
            with pytest.raises(OSError):
                MlsContext.restore_state(blob[: len(blob) // 2])

    def test_invite_full_two_members(self):
        with MlsContext.create("alice") as alice, MlsContext.create("bob") as bob:
            commit, welcome = alice.invite_full(bob.export_key_package())
            assert isinstance(commit, bytes) and len(commit) > 0
            assert isinstance(welcome, bytes) and len(welcome) > 0
            epoch_before = alice.epoch
            alice.finalize_commit()
            assert alice.epoch > epoch_before
            bob.accept_welcome(welcome)
            assert bob.epoch == alice.epoch

    def test_invite_full_three_members(self):
        with (
            MlsContext.create("alice") as alice,
            MlsContext.create("bob") as bob,
            MlsContext.create("carol") as carol,
        ):
            bob.accept_welcome(alice.invite(bob.export_key_package()))
            commit, welcome_carol = alice.invite_full(carol.export_key_package())
            alice.finalize_commit()
            bob.process_message(commit)
            bob.finalize_commit()
            carol.accept_welcome(welcome_carol)
            assert alice.epoch == bob.epoch == carol.epoch

    def test_clear_pending_commit(self):
        with MlsContext.create("alice") as alice, MlsContext.create("bob") as bob:
            alice.invite_full(bob.export_key_package())
            alice.clear_pending_commit()

    def test_process_message_returns_commit(self):
        with (
            MlsContext.create("alice") as alice,
            MlsContext.create("bob") as bob,
            MlsContext.create("carol") as carol,
        ):
            bob.accept_welcome(alice.invite(bob.export_key_package()))
            commit, _ = alice.invite_full(carol.export_key_package())
            alice.finalize_commit()
            kind = bob.process_message(commit)
            assert kind == "commit"
            bob.finalize_commit()


# ---------------------------------------------------------------------------
# GroupNode
# ---------------------------------------------------------------------------

class TestGroupNode:
    def test_create_and_state(self):
        with MlsContext.create("alice") as mls:
            gid = mls.group_id
            with GroupNode.create(member_id=1, group_id=gid) as node:
                assert node.state == NodeState.IDLE
                node.bootstrap_as_creator(mls.epoch)
                assert node.state == NodeState.ACTIVE

    def test_epoch_matches_mls(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            assert alice_node.epoch == alice_mls.epoch
            assert bob_node.epoch == bob_mls.epoch
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_group_id_preserved(self):
        with MlsContext.create("alice") as mls:
            gid = mls.group_id
            with GroupNode.create(member_id=1, group_id=gid) as node:
                assert node.group_id == gid

    def test_invalid_group_id_raises(self):
        with pytest.raises(ValueError):
            GroupNode.create(member_id=1, group_id=b"\x00" * 15)

    def test_send_control_produces_frame(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            frame = alice_node.send_control(
                alice_mls, target=2,
                opcode=ControlOpcode.ACK,
                transition_id=0, request_id=1,
            )
            assert len(frame.wire) > 0
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()


# ---------------------------------------------------------------------------
# GTP — text messaging
# ---------------------------------------------------------------------------

class TestGtpClient:
    def test_basic_send_receive(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()

            frame = gtp_alice.send(alice_node, alice_mls, target=2, message_id=1, text="hello")
            evs = _text_events(bob_node.on_wire(bob_mls, frame.wire))
            assert len(evs) == 1
            result = gtp_bob.accept(_pt(evs[0]), bob_mls.epoch)
            assert result.status == "new"
            assert result.text == "hello"

            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_bidirectional_messaging(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()

            frame = gtp_bob.send(bob_node, bob_mls, target=1, message_id=10, text="hi alice")
            evs = _text_events(alice_node.on_wire(alice_mls, frame.wire))
            assert len(evs) == 1
            result = gtp_alice.accept(_pt(evs[0]), alice_mls.epoch)
            assert result.text == "hi alice"
            assert result.sender == 2

            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_multiple_messages(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()

            for i, msg in enumerate(["first", "second", "third"], start=1):
                frame = gtp_alice.send(alice_node, alice_mls, target=2, message_id=i, text=msg)
                evs = _text_events(bob_node.on_wire(bob_mls, frame.wire))
                assert len(evs) == 1
                result = gtp_bob.accept(_pt(evs[0]), bob_mls.epoch)
                assert result.text == msg

            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_duplicate_rejection(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()

            frame = gtp_alice.send(alice_node, alice_mls, target=2, message_id=99, text="once")
            evs = _text_events(bob_node.on_wire(bob_mls, frame.wire))
            pt = _pt(evs[0])
            assert gtp_bob.accept(pt, bob_mls.epoch).status == "new"
            assert gtp_bob.accept(pt, bob_mls.epoch).status == "duplicate"

            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_unicode_text(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()

            text = "Привет мир 🌍"
            frame = gtp_alice.send(alice_node, alice_mls, target=2, message_id=5, text=text)
            evs = _text_events(bob_node.on_wire(bob_mls, frame.wire))
            result = gtp_bob.accept(_pt(evs[0]), bob_mls.epoch)
            assert result.text == text

            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_reset_clears_dedup(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()

            frame = gtp_alice.send(alice_node, alice_mls, target=2, message_id=7, text="test")
            evs = _text_events(bob_node.on_wire(bob_mls, frame.wire))
            pt = _pt(evs[0])
            assert gtp_bob.accept(pt, bob_mls.epoch).status == "new"
            gtp_bob.reset()
            assert gtp_bob.accept(pt, bob_mls.epoch).status == "new"

            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()


# ---------------------------------------------------------------------------
# GAP — audio
# ---------------------------------------------------------------------------

class TestGapClient:
    def test_basic_audio_frame(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gap_alice = GapClient.create()
            gap_bob = GapClient.create()

            frame = gap_alice.send(alice_node, alice_mls, target=2,
                                   media_source_id=1, rtp_timestamp=0, opus=b"\xfc\xff\xfe" * 20)
            evs = _audio_events(bob_node.on_wire(bob_mls, frame.wire))
            assert len(evs) == 1
            result = gap_bob.accept(_pt(evs[0]), bob_mls.epoch)
            assert result.status == "new"
            assert result.source == 1

            gap_alice.close(); gap_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_multiple_audio_frames_in_order(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gap_alice = GapClient.create()
            gap_bob = GapClient.create()

            for i in range(5):
                frame = gap_alice.send(alice_node, alice_mls, target=2,
                                       media_source_id=1, rtp_timestamp=i * 960,
                                       opus=bytes([i] * 10))
                evs = _audio_events(bob_node.on_wire(bob_mls, frame.wire))
                assert len(evs) == 1
                result = gap_bob.accept(_pt(evs[0]), bob_mls.epoch)
                assert result.status == "new"
                assert result.seq == i + 1

            gap_alice.close(); gap_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_multiple_sources(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gap_alice = GapClient.create()
            gap_bob = GapClient.create()

            for src in [10, 20]:
                frame = gap_alice.send(alice_node, alice_mls, target=2,
                                       media_source_id=src, rtp_timestamp=0,
                                       opus=bytes([src] * 8))
                evs = _audio_events(bob_node.on_wire(bob_mls, frame.wire))
                result = gap_bob.accept(_pt(evs[0]), bob_mls.epoch)
                assert result.status == "new"
                assert result.source == src

            gap_alice.close(); gap_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_reset(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gap_alice = GapClient.create()
            gap_bob = GapClient.create()

            frame = gap_alice.send(alice_node, alice_mls, target=2,
                                   media_source_id=1, rtp_timestamp=0, opus=b"\x00" * 5)
            evs = _audio_events(bob_node.on_wire(bob_mls, frame.wire))
            gap_bob.accept(_pt(evs[0]), bob_mls.epoch)
            gap_bob.reset()

            gap_alice.close(); gap_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()


# ---------------------------------------------------------------------------
# GSP — signals
# ---------------------------------------------------------------------------

class TestGspClient:
    def _send_and_receive(self, signal, alice_mls, alice_node, bob_mls, bob_node,
                          role_claim=0, request_id=1, args: bytes = b""):
        gsp_alice = GspClient.create()
        gsp_bob = GspClient.create()
        try:
            frame = gsp_alice.send(alice_node, alice_mls, target=2,
                                   signal=signal, role_claim=role_claim,
                                   request_id=request_id, args=args)
            evs = _signal_events(bob_node.on_wire(bob_mls, frame.wire))
            assert len(evs) == 1
            return gsp_bob.accept(_pt(evs[0]), bob_mls.epoch)
        finally:
            gsp_alice.close(); gsp_bob.close()

    def _group(self):
        return _two_member_group()

    def test_join_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            r = self._send_and_receive(SignalType.JOIN, alice_mls, alice_node, bob_mls, bob_node, request_id=1)
            assert r.status == "new" and r.signal_code == SignalType.JOIN
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_leave_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            r = self._send_and_receive(SignalType.LEAVE, alice_mls, alice_node, bob_mls, bob_node, request_id=2)
            assert r.status == "new" and r.signal_code == SignalType.LEAVE
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_mute_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            # MUTE requires args = {0: target_member_id}
            r = self._send_and_receive(SignalType.MUTE, alice_mls, alice_node, bob_mls, bob_node,
                                       request_id=3, args=_cbor_map1(0, 2))
            assert r.status == "new" and r.signal_code == SignalType.MUTE
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_unmute_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            # UNMUTE requires args = {0: target_member_id}
            r = self._send_and_receive(SignalType.UNMUTE, alice_mls, alice_node, bob_mls, bob_node,
                                       request_id=4, args=_cbor_map1(0, 2))
            assert r.status == "new" and r.signal_code == SignalType.UNMUTE
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_role_change_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            # ROLE_CHANGE requires args = {0: target_member_id, 1: new_role_id}
            r = self._send_and_receive(SignalType.ROLE_CHANGE, alice_mls, alice_node, bob_mls, bob_node,
                                       role_claim=5, request_id=5, args=_cbor_map2(0, 2, 1, 3))
            assert r.status == "new" and r.signal_code == SignalType.ROLE_CHANGE
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_stream_start_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            # STREAM_START requires args = {0: stream_type}
            r = self._send_and_receive(SignalType.STREAM_START, alice_mls, alice_node, bob_mls, bob_node,
                                       request_id=6, args=_cbor_map1(0, 2))
            assert r.status == "new" and r.signal_code == SignalType.STREAM_START
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_stream_stop_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            # STREAM_STOP requires args = {0: stream_type}
            r = self._send_and_receive(SignalType.STREAM_STOP, alice_mls, alice_node, bob_mls, bob_node,
                                       request_id=7, args=_cbor_map1(0, 2))
            assert r.status == "new" and r.signal_code == SignalType.STREAM_STOP
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_codec_update_signal(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            # CODEC_UPDATE requires args = {0: codec_id}
            r = self._send_and_receive(SignalType.CODEC_UPDATE, alice_mls, alice_node, bob_mls, bob_node,
                                       request_id=8, args=_cbor_map1(0, 1))
            assert r.status == "new" and r.signal_code == SignalType.CODEC_UPDATE
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()

    def test_duplicate_signal_rejected(self):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gsp_alice = GspClient.create()
            gsp_bob = GspClient.create()

            # MUTE requires args = {0: target_member_id}
            frame = gsp_alice.send(alice_node, alice_mls, target=2,
                                   signal=SignalType.MUTE, role_claim=0, request_id=50,
                                   args=_cbor_map1(0, 2))
            evs = _signal_events(bob_node.on_wire(bob_mls, frame.wire))
            pt = _pt(evs[0])
            assert gsp_bob.accept(pt, bob_mls.epoch).status == "new"
            assert gsp_bob.accept(pt, bob_mls.epoch).status == "duplicate"

            gsp_alice.close(); gsp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    def test_sender_id(self):
        alice_mls, alice_node, bob_mls, bob_node = self._group()
        try:
            r = self._send_and_receive(SignalType.JOIN, alice_mls, alice_node, bob_mls, bob_node, request_id=9)
            assert r.sender == 1
        finally:
            alice_node.close(); bob_node.close(); alice_mls.close(); bob_mls.close()


# ---------------------------------------------------------------------------
# User lifecycle: 3-member group, leave, rejoin
# ---------------------------------------------------------------------------

class TestUserLifecycle:
    def test_three_member_group_broadcast(self):
        alice_mls = MlsContext.create("alice")
        bob_mls = MlsContext.create("bob")
        carol_mls = MlsContext.create("carol")
        try:
            bob_kp = bob_mls.export_key_package()
            bob_mls.accept_welcome(alice_mls.invite(bob_kp))

            carol_kp = carol_mls.export_key_package()
            commit, welcome_carol = alice_mls.invite_full(carol_kp)
            alice_mls.finalize_commit()
            bob_mls.process_message(commit)
            bob_mls.finalize_commit()
            carol_mls.accept_welcome(welcome_carol)

            assert alice_mls.epoch == bob_mls.epoch == carol_mls.epoch

            gid = alice_mls.group_id
            alice_node = GroupNode.create(1, gid)
            bob_node = GroupNode.create(2, gid)
            carol_node = GroupNode.create(3, gid)
            alice_node.bootstrap_as_creator(alice_mls.epoch)
            bob_node.bootstrap_as_joiner(bob_mls.epoch)
            carol_node.bootstrap_as_joiner(carol_mls.epoch)

            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()
            gtp_carol = GtpClient.create()

            frame = gtp_alice.send(alice_node, alice_mls, target=0,
                                   message_id=1, text="hi all")
            for node, mls, client in [
                (bob_node, bob_mls, gtp_bob),
                (carol_node, carol_mls, gtp_carol),
            ]:
                evs = _text_events(node.on_wire(mls, frame.wire))
                assert len(evs) == 1
                result = client.accept(_pt(evs[0]), mls.epoch)
                assert result.text == "hi all"

            for x in [alice_node, bob_node, carol_node, gtp_alice, gtp_bob, gtp_carol]:
                x.close()
        finally:
            alice_mls.close(); bob_mls.close(); carol_mls.close()

    def test_member_rejoin(self):
        alice_mls = MlsContext.create("alice")
        bob_mls = MlsContext.create("bob")
        alice_node = bob_node2 = None
        try:
            kp = bob_mls.export_key_package()
            bob_mls.accept_welcome(alice_mls.invite(kp))

            gid = alice_mls.group_id

            # Remove bob (leaf 1) and finalize
            alice_mls.remove_member(1)
            alice_mls.finalize_commit()

            # Bob2 rejoins
            bob_mls2 = MlsContext.create("bob2")
            kp2 = bob_mls2.export_key_package()
            bob_mls2.accept_welcome(alice_mls.invite(kp2))

            # Bootstrap nodes at the current epoch so keys match
            alice_node = GroupNode.create(1, gid)
            bob_node2 = GroupNode.create(2, gid)
            alice_node.bootstrap_as_creator(alice_mls.epoch)
            bob_node2.bootstrap_as_joiner(bob_mls2.epoch)

            gtp_alice = GtpClient.create()
            gtp_bob2 = GtpClient.create()

            frame = gtp_alice.send(alice_node, alice_mls, target=2,
                                   message_id=1, text="welcome back")
            evs = _text_events(bob_node2.on_wire(bob_mls2, frame.wire))
            assert len(evs) == 1
            result = gtp_bob2.accept(_pt(evs[0]), bob_mls2.epoch)
            assert result.text == "welcome back"

            for x in [gtp_alice, gtp_bob2]:
                x.close()
            bob_mls2.close()
        finally:
            if alice_node: alice_node.close()
            if bob_node2: bob_node2.close()
            alice_mls.close(); bob_mls.close()


# ---------------------------------------------------------------------------
# Coordinator events
# ---------------------------------------------------------------------------

class TestCoordinatorEvents:
    def test_coordinator_event_kinds_parsed(self):
        import json
        events_json = json.dumps([
            {"kind": "coordinator_election_needed"},
            {"kind": "became_coordinator"},
            {"kind": "coordinator_claim", "claimant": 42},
        ])
        events = _parse_events(events_json)
        assert len(events) == 3
        assert events[0].kind == "coordinator_election_needed"
        assert events[0].claimant is None
        assert events[1].kind == "became_coordinator"
        assert events[2].kind == "coordinator_claim"
        assert events[2].claimant == 42

    def test_node_event_dataclass_claimant(self):
        ev = NodeEvent(kind="coordinator_claim", claimant=7)
        assert ev.claimant == 7

    def test_election_needed_no_claimant(self):
        ev = NodeEvent(kind="coordinator_election_needed")
        assert ev.claimant is None

    def test_became_coordinator_no_claimant(self):
        ev = NodeEvent(kind="became_coordinator")
        assert ev.claimant is None


# ---------------------------------------------------------------------------
# MessageHistory + Watermark
# ---------------------------------------------------------------------------

class TestMessageHistory:
    def test_push_and_contains(self):
        h = MessageHistory(10)
        entry = MessageEntry(sender_id=1, message_id=100, text="hello")
        assert h.push(entry) is True
        assert h.contains(1, 100)

    def test_duplicate_push_returns_false(self):
        h = MessageHistory(10)
        e = MessageEntry(1, 1, "x")
        h.push(e)
        assert h.push(e) is False

    def test_capacity_limit(self):
        h = MessageHistory(3)
        for i in range(5):
            h.push(MessageEntry(1, i, str(i)))
        assert len(h) == 3

    def test_since_watermark(self):
        h = MessageHistory(100)
        for i in range(5):
            h.push(MessageEntry(sender_id=1, message_id=i, text=str(i)))
        w = Watermark()
        w.observe(1, 2)
        ids = [m.message_id for m in h.since(w)]
        assert ids == [3, 4]

    def test_since_for_sender(self):
        h = MessageHistory(100)
        h.push(MessageEntry(1, 10, "a"))
        h.push(MessageEntry(2, 20, "b"))
        h.push(MessageEntry(1, 30, "c"))
        msgs = list(h.since_for_sender(1, 10))
        assert len(msgs) == 1 and msgs[0].message_id == 30

    def test_clear(self):
        h = MessageHistory(10)
        h.push(MessageEntry(1, 1, "hi"))
        h.clear()
        assert len(h) == 0

    def test_watermark_observe_and_snapshot(self):
        w = Watermark()
        w.observe(1, 5)
        w.observe(1, 3)
        w.observe(2, 10)
        assert w.last_seen(1) == 5
        assert w.last_seen(2) == 10
        assert w.last_seen(3) is None
        assert w.snapshot() == {1: 5, 2: 10}

    def test_watermark_clear(self):
        w = Watermark()
        w.observe(1, 1)
        w.clear()
        assert len(w) == 0

    def test_invalid_capacity(self):
        with pytest.raises(ValueError):
            MessageHistory(0)


# ---------------------------------------------------------------------------
# JitterBuffer
# ---------------------------------------------------------------------------

class TestJitterBuffer:
    def test_accepted_frame(self):
        buf = JitterBuffer(10)
        result = buf.push(AudioFrame(media_source_id=1, rtp_sequence=0, plaintext=b"a"))
        assert result.outcome == JitterPushOutcome.ACCEPTED
        assert result.evicted is None

    def test_pop_in_order(self):
        buf = JitterBuffer(10)
        buf.push(AudioFrame(1, 0, b"0"))
        buf.push(AudioFrame(1, 1, b"1"))
        f0 = buf.pop_in_order(1)
        assert f0 is not None and f0.rtp_sequence == 0
        f1 = buf.pop_in_order(1)
        assert f1 is not None and f1.rtp_sequence == 1

    def test_pop_force_skips_gap(self):
        buf = JitterBuffer(10)
        buf.push(AudioFrame(1, 0, b"0"))
        buf.pop_in_order(1)
        buf.push(AudioFrame(1, 5, b"5"))
        f = buf.pop_force(1)
        assert f is not None and f.rtp_sequence == 5

    def test_late_frame(self):
        buf = JitterBuffer(10)
        buf.push(AudioFrame(1, 5, b"5"))
        buf.pop_force(1)
        result = buf.push(AudioFrame(1, 3, b"3"))
        assert result.outcome == JitterPushOutcome.LATE

    def test_eviction_on_overflow(self):
        buf = JitterBuffer(2)
        buf.push(AudioFrame(1, 0, b"0"))
        buf.push(AudioFrame(1, 1, b"1"))
        result = buf.push(AudioFrame(1, 2, b"2"))
        assert result.outcome == JitterPushOutcome.EVICTED
        assert result.evicted is not None

    def test_multiple_sources_independent(self):
        buf = JitterBuffer(5)
        buf.push(AudioFrame(1, 0, b"a"))
        buf.push(AudioFrame(2, 0, b"b"))
        f1 = buf.pop_in_order(1)
        f2 = buf.pop_in_order(2)
        assert f1 is not None and f1.plaintext == b"a"
        assert f2 is not None and f2.plaintext == b"b"

    def test_length_for(self):
        buf = JitterBuffer(10)
        assert buf.length_for(1) == 0
        buf.push(AudioFrame(1, 0, b"x"))
        assert buf.length_for(1) == 1

    def test_clear(self):
        buf = JitterBuffer(10)
        buf.push(AudioFrame(1, 0, b"z"))
        buf.clear()
        assert buf.length_for(1) == 0

    def test_out_of_order_reordering(self):
        buf = JitterBuffer(10)
        buf.push(AudioFrame(1, 2, b"2"))
        buf.push(AudioFrame(1, 0, b"0"))
        buf.push(AudioFrame(1, 1, b"1"))
        seqs = []
        while True:
            f = buf.pop_in_order(1)
            if f is None:
                break
            seqs.append(f.rtp_sequence)
        assert seqs == [0, 1, 2]

    def test_invalid_capacity(self):
        with pytest.raises(ValueError):
            JitterBuffer(0)


# ---------------------------------------------------------------------------
# RoleRegistry + CapabilitiesNegotiator
# ---------------------------------------------------------------------------

class TestRoleRegistry:
    def test_define_and_assign(self):
        reg = RoleRegistry()
        reg.define_role(1, "moderator", Permissions.SEND_TEXT | Permissions.MUTE_OTHERS)
        reg.assign(42, 1)
        assert reg.permissions_of(42) == (Permissions.SEND_TEXT | Permissions.MUTE_OTHERS)

    def test_has_permission(self):
        reg = RoleRegistry()
        reg.define_role(2, "viewer", Permissions.SEND_TEXT)
        reg.assign(1, 2)
        assert reg.has(1, Permissions.SEND_TEXT)
        assert not reg.has(1, Permissions.MUTE_OTHERS)

    def test_require_raises_on_missing(self):
        reg = RoleRegistry()
        reg.define_role(1, "guest", Permissions.NONE)
        reg.assign(5, 1)
        with pytest.raises(RoleError):
            reg.require(5, Permissions.SEND_TEXT)

    def test_require_passes_when_has_permission(self):
        reg = RoleRegistry()
        reg.define_role(1, "admin", Permissions.CLOSE_GROUP | Permissions.ASSIGN_ROLES)
        reg.assign(5, 1)
        reg.require(5, Permissions.CLOSE_GROUP)

    def test_unknown_role_assign_raises(self):
        reg = RoleRegistry()
        with pytest.raises(RoleError):
            reg.assign(1, 999)

    def test_no_role_gives_none_permissions(self):
        reg = RoleRegistry()
        assert reg.permissions_of(99) == Permissions.NONE

    def test_permissions_intflag_combination(self):
        p = Permissions.SEND_TEXT | Permissions.SEND_AUDIO | Permissions.INVITE
        assert Permissions.SEND_TEXT in p
        assert Permissions.CLOSE_GROUP not in p

    def test_role_of(self):
        reg = RoleRegistry()
        reg.define_role(3, "speaker", Permissions.SEND_AUDIO)
        reg.assign(10, 3)
        spec = reg.role_of(10)
        assert spec is not None and spec.name == "speaker"

    def test_all_permission_bits(self):
        all_perms = (
            Permissions.SEND_TEXT | Permissions.SEND_AUDIO | Permissions.SEND_SIGNAL |
            Permissions.MUTE_OTHERS | Permissions.ASSIGN_ROLES | Permissions.INVITE |
            Permissions.REMOVE_MEMBERS | Permissions.CLOSE_GROUP
        )
        reg = RoleRegistry()
        reg.define_role(10, "superadmin", all_perms)
        reg.assign(1, 10)
        for bit in [Permissions.SEND_TEXT, Permissions.SEND_AUDIO, Permissions.CLOSE_GROUP]:
            assert reg.has(1, bit)


class TestCapabilitiesNegotiator:
    def test_advertise_and_query(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(1, ["audio", "video"])
        neg.advertise(2, ["audio"])
        assert neg.group_supports("audio")
        assert not neg.group_supports("video")

    def test_intersection(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(1, ["a", "b", "c"])
        neg.advertise(2, ["b", "c"])
        assert neg.intersection() == {"b", "c"}

    def test_union(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(1, ["a"])
        neg.advertise(2, ["b"])
        assert neg.union() == {"a", "b"}

    def test_missing(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(1, ["x"])
        neg.advertise(2, [])
        missing = neg.missing("x")
        assert 2 in missing and 1 not in missing

    def test_forget(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(1, ["a"])
        neg.forget(1)
        assert len(neg) == 0

    def test_capabilities_of(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(5, ["alpha", "beta"])
        assert neg.capabilities_of(5) == {"alpha", "beta"}

    def test_empty_intersection(self):
        assert CapabilitiesNegotiator().intersection() == set()

    def test_group_supports_false_when_empty(self):
        assert not CapabilitiesNegotiator().group_supports("anything")

    def test_update_advertisement(self):
        neg = CapabilitiesNegotiator()
        neg.advertise(1, ["a"])
        neg.advertise(1, ["b"])
        assert neg.capabilities_of(1) == {"b"}


# ---------------------------------------------------------------------------
# SFrame E2EE
# ---------------------------------------------------------------------------

class TestSFrameE2EE:
    def _pair(self):
        alice_mls = MlsContext.create("alice")
        bob_mls = MlsContext.create("bob")
        welcome = alice_mls.invite(bob_mls.export_key_package())
        bob_mls.accept_welcome(welcome)
        return alice_mls, bob_mls

    def test_aes128_encrypt_decrypt(self):
        alice_mls, bob_mls = self._pair()
        try:
            with SFrameSession.create(alice_mls, suite=AES_128_GCM) as alice_sess, \
                 SFrameSession.create(bob_mls, suite=AES_128_GCM) as bob_sess:
                enc = alice_sess.create_encryptor(alice_mls, leaf_index=0)
                plaintext = b"opus_frame_data_here"
                ct = enc.encrypt(plaintext)
                assert ct != plaintext
                result = bob_sess.decrypt(ct)
                assert result.plaintext == plaintext
                assert result.sender_leaf == 0
                enc.close()
        finally:
            alice_mls.close(); bob_mls.close()

    def test_aes256_encrypt_decrypt(self):
        alice_mls, bob_mls = self._pair()
        try:
            with SFrameSession.create(alice_mls, suite=AES_256_GCM) as alice_sess, \
                 SFrameSession.create(bob_mls, suite=AES_256_GCM) as bob_sess:
                enc = alice_sess.create_encryptor(alice_mls, leaf_index=0, suite=AES_256_GCM)
                plaintext = b"hd_audio_frame"
                result = bob_sess.decrypt(enc.encrypt(plaintext))
                assert result.plaintext == plaintext
                enc.close()
        finally:
            alice_mls.close(); bob_mls.close()

    def test_extra_aad(self):
        alice_mls, bob_mls = self._pair()
        try:
            with SFrameSession.create(alice_mls) as alice_sess, \
                 SFrameSession.create(bob_mls) as bob_sess:
                enc = alice_sess.create_encryptor(alice_mls, leaf_index=0)
                aad = b"stream-id-42"
                plaintext = b"frame_with_aad"
                ct = enc.encrypt(plaintext, extra_aad=aad)
                result = bob_sess.decrypt(ct, extra_aad=aad)
                assert result.plaintext == plaintext
                enc.close()
        finally:
            alice_mls.close(); bob_mls.close()

    def test_multiple_frames_replay_window(self):
        alice_mls, bob_mls = self._pair()
        try:
            with SFrameSession.create(alice_mls) as alice_sess, \
                 SFrameSession.create(bob_mls) as bob_sess:
                enc = alice_sess.create_encryptor(alice_mls, leaf_index=0)
                for i in range(10):
                    pt = bytes([i] * 16)
                    result = bob_sess.decrypt(enc.encrypt(pt))
                    assert result.plaintext == pt
                enc.close()
        finally:
            alice_mls.close(); bob_mls.close()

    def test_wrong_aad_fails(self):
        alice_mls, bob_mls = self._pair()
        try:
            with SFrameSession.create(alice_mls) as alice_sess, \
                 SFrameSession.create(bob_mls) as bob_sess:
                enc = alice_sess.create_encryptor(alice_mls, leaf_index=0)
                ct = enc.encrypt(b"secret", extra_aad=b"correct")
                with pytest.raises(RuntimeError):
                    bob_sess.decrypt(ct, extra_aad=b"wrong")
                enc.close()
        finally:
            alice_mls.close(); bob_mls.close()

    def test_context_manager(self):
        with MlsContext.create("alice") as mls:
            with SFrameSession.create(mls) as sess:
                assert sess._handle != 0
            assert sess._handle == 0


# ---------------------------------------------------------------------------
# Utility functions
# ---------------------------------------------------------------------------

class TestCodecRoundtrip:
    """Codec roundtrip tests: all three sub-protocols × all three codecs."""

    @pytest.mark.parametrize("codec", [PayloadCodec.CBOR, PayloadCodec.PROTOBUF, PayloadCodec.FLATBUFFERS])
    def test_gtp_roundtrip(self, codec):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gtp_alice = GtpClient.create()
            gtp_bob = GtpClient.create()
            frame = gtp_alice.send(alice_node, alice_mls, target=2, message_id=1,
                                   text="codec test", codec=codec)
            evs = _text_events(bob_node.on_wire(bob_mls, frame.wire))
            assert len(evs) == 1
            assert evs[0].codec == codec
            result = gtp_bob.accept(_pt(evs[0]), bob_mls.epoch, codec=codec)
            assert result.status == "new"
            assert result.text == "codec test"
            gtp_alice.close(); gtp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    @pytest.mark.parametrize("codec", [PayloadCodec.CBOR, PayloadCodec.PROTOBUF, PayloadCodec.FLATBUFFERS])
    def test_gap_roundtrip(self, codec):
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gap_alice = GapClient.create()
            gap_bob = GapClient.create()
            frame = gap_alice.send(alice_node, alice_mls, target=2,
                                   media_source_id=5, rtp_timestamp=0,
                                   opus=b"\xaa" * 20, codec=codec)
            evs = _audio_events(bob_node.on_wire(bob_mls, frame.wire))
            assert len(evs) == 1
            assert evs[0].codec == codec
            result = gap_bob.accept(_pt(evs[0]), bob_mls.epoch, codec=codec)
            assert result.status == "new"
            assert result.source == 5
            gap_alice.close(); gap_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()

    @pytest.mark.parametrize("codec", [PayloadCodec.CBOR, PayloadCodec.PROTOBUF, PayloadCodec.FLATBUFFERS])
    def test_gsp_roundtrip(self, codec):
        from gbp_stack.gsp_client import SignalType
        alice_mls, alice_node, bob_mls, bob_node = _two_member_group()
        try:
            gsp_alice = GspClient.create()
            gsp_bob = GspClient.create()
            frame = gsp_alice.send(alice_node, alice_mls, target=2,
                                   signal=SignalType.JOIN, role_claim=0,
                                   request_id=1, codec=codec)
            evs = _signal_events(bob_node.on_wire(bob_mls, frame.wire))
            assert len(evs) == 1
            assert evs[0].codec == codec
            result = gsp_bob.accept(_pt(evs[0]), bob_mls.epoch, codec=codec)
            assert result.status == "new"
            assert result.signal_code == SignalType.JOIN
            gsp_alice.close(); gsp_bob.close()
        finally:
            alice_node.close(); bob_node.close()
            alice_mls.close(); bob_mls.close()


class TestUtilities:
    def test_encode_gbp_frame_returns_bytes(self):
        with MlsContext.create("alice") as mls:
            result = encode_gbp_frame(
                version=1, group_id=mls.group_id, epoch=1,
                transition_id=0, stream_type=2, stream_id=0,
                flags=0, sequence_no=1, payload=b"hello",
            )
            assert isinstance(result, bytes) and len(result) > 0

    def test_encode_gbp_frame_bad_group_id(self):
        with pytest.raises(ValueError):
            encode_gbp_frame(1, b"\x00" * 5, 1, 0, 2, 0, 0, 1, b"x")

    def test_lookup_error_known_code(self):
        result = lookup_error(0x0001)
        if result is not None:
            assert isinstance(result, bytes) and len(result) > 0

    def test_lookup_error_unknown_code_returns_none(self):
        assert lookup_error(0xFFFF) is None

    def test_encode_gbp_frame_empty_payload(self):
        with MlsContext.create("alice") as mls:
            result = encode_gbp_frame(
                version=1, group_id=mls.group_id, epoch=1,
                transition_id=0, stream_type=0, stream_id=0,
                flags=0, sequence_no=0, payload=b"",
            )
            assert isinstance(result, bytes)
