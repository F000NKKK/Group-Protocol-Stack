"""Two-party GTP (text) chat — minimal in-memory end-to-end example.

Demonstrates:
  - MLS two-party handshake
  - GBP node bootstrap
  - Sending text messages with CBOR and FlatBuffers codecs
  - Idempotency: duplicate (sender, message_id) returns status "duplicate"

Run:
    python python/examples/gtp_chat.py
"""

from gbp_stack import MlsContext, GroupNode, GtpClient, PayloadCodec

# --- MLS handshake -----------------------------------------------------------
with MlsContext.create("alice") as alice_mls, \
     MlsContext.create("bob")   as bob_mls:

    bob_kp = bob_mls.export_key_package()
    bob_mls.accept_welcome(alice_mls.invite(bob_kp))
    print(f"MLS epoch: alice={alice_mls.epoch}  bob={bob_mls.epoch}")

    # --- GBP nodes -----------------------------------------------------------
    gid = alice_mls.group_id
    with GroupNode.create(member_id=1, group_id=gid) as alice, \
         GroupNode.create(member_id=2, group_id=gid) as bob, \
         GtpClient.create() as gtp_alice, \
         GtpClient.create() as gtp_bob:

        alice.bootstrap_as_creator(alice_mls.epoch)
        bob.bootstrap_as_joiner(bob_mls.epoch)

        # Send "hello" with default CBOR codec.
        frame = gtp_alice.send(alice, alice_mls, target=2, message_id=1, text="hello")
        for ev in bob.on_wire(bob_mls, frame.wire):
            if ev.kind == "payload_received" and ev.stream_type == 2:
                r = gtp_bob.accept(ev.plaintext or b"", bob_mls.epoch,
                                   codec=ev.codec or PayloadCodec.CBOR)
                print(f"new (cbor):   text={r.text!r}  status={r.status}")

        # Send with FlatBuffers codec.
        frame2 = gtp_alice.send(alice, alice_mls, target=2, message_id=2,
                                text="hello flatbuffers",
                                codec=PayloadCodec.FLATBUFFERS)
        for ev in bob.on_wire(bob_mls, frame2.wire):
            if ev.kind == "payload_received" and ev.stream_type == 2:
                r = gtp_bob.accept(ev.plaintext or b"", bob_mls.epoch,
                                   codec=ev.codec or PayloadCodec.CBOR)
                print(f"new (fbs):    text={r.text!r}  codec={ev.codec}")

        # Replay: same message_id=1 must come back as "duplicate".
        dup = gtp_alice.send(alice, alice_mls, target=2, message_id=1, text="hello")
        for ev in bob.on_wire(bob_mls, dup.wire):
            if ev.kind == "payload_received" and ev.stream_type == 2:
                r = gtp_bob.accept(ev.plaintext, bob_mls.epoch)
                print(f"replay:       status={r.status}")  # → duplicate
