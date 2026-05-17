"""GSP (signaling) examples: JOIN, MUTE with CBOR args, ROLE_CHANGE.

Demonstrates:
  - Signals without args via GspClient.send
  - Signals with per-signal CBOR args via GspClient.send (args= keyword)

Run:
    python python/examples/gsp_signals.py
"""

from gbp_stack import MlsContext, GroupNode, GspClient, PayloadCodec
from gbp_stack.gsp_client import SignalType


def cbor_uint(n: int) -> bytes:
    if n <= 23:     return bytes([n])
    if n <= 0xFF:   return bytes([0x18, n])
    if n <= 0xFFFF: return bytes([0x19, n >> 8, n & 0xFF])
    return bytes([0x1A, (n>>24)&0xFF, (n>>16)&0xFF, (n>>8)&0xFF, n&0xFF])

def cbor_map1(k: int, v: int) -> bytes:
    return bytes([0xA1]) + cbor_uint(k) + cbor_uint(v)

def cbor_map2(k0: int, v0: int, k1: int, v1: int) -> bytes:
    return bytes([0xA2]) + cbor_uint(k0) + cbor_uint(v0) + cbor_uint(k1) + cbor_uint(v1)


with MlsContext.create("alice") as alice_mls, \
     MlsContext.create("bob")   as bob_mls:

    bob_mls.accept_welcome(alice_mls.invite(bob_mls.export_key_package()))

    gid = alice_mls.group_id
    with GroupNode.create(member_id=1, group_id=gid) as alice, \
         GroupNode.create(member_id=2, group_id=gid) as bob, \
         GspClient.create() as gsp_alice, \
         GspClient.create() as gsp_bob:

        alice.bootstrap_as_creator(alice_mls.epoch)
        bob.bootstrap_as_joiner(bob_mls.epoch)

        def recv(wire: bytes, label: str) -> None:
            for ev in bob.on_wire(bob_mls, wire):
                if ev.kind == "payload_received" and ev.stream_type == 3:
                    assert ev.plaintext is not None
                    r = gsp_bob.accept(ev.plaintext, bob_mls.epoch,
                                      codec=ev.codec or PayloadCodec.CBOR)
                    print(f"{label}: signal={r.signal}  sender={r.sender}  request_id={r.request_id}")

        # 1. JOIN — no args.
        recv(gsp_alice.send(alice, alice_mls, target=0,
                            signal=SignalType.JOIN, role_claim=0, request_id=1).wire, "JOIN")

        # 2. MUTE member 2 — args: {0: target_member_id=2}.
        recv(gsp_alice.send(alice, alice_mls, target=0,
                            signal=SignalType.MUTE, role_claim=0, request_id=2,
                            args=cbor_map1(0, 2)).wire, "MUTE")

        # 3. ROLE_CHANGE member 2 → role 1 — args: {0: target=2, 1: new_role=1}.
        recv(gsp_alice.send(alice, alice_mls, target=0,
                            signal=SignalType.ROLE_CHANGE, role_claim=1, request_id=3,
                            args=cbor_map2(0, 2, 1, 1)).wire, "ROLE_CHANGE")
