"""Two-party GAP (audio) frame exchange — minimal in-memory end-to-end example.

Demonstrates:
  - Sending synthetic Opus frames with FlatBuffers codec (recommended for audio)
  - Sequential frames: rtp_sequence advances automatically inside GapClient

Run:
    python python/examples/gap_audio.py
"""

from gbp_stack import MlsContext, GroupNode, GapClient, PayloadCodec

with MlsContext.create("alice") as alice_mls, \
     MlsContext.create("bob")   as bob_mls:

    bob_mls.accept_welcome(alice_mls.invite(bob_mls.export_key_package()))

    gid = alice_mls.group_id
    with GroupNode.create(member_id=1, group_id=gid) as alice, \
         GroupNode.create(member_id=2, group_id=gid) as bob, \
         GapClient.create() as gap_alice, \
         GapClient.create() as gap_bob:

        alice.bootstrap_as_creator(alice_mls.epoch)
        bob.bootstrap_as_joiner(bob_mls.epoch)

        # Synthetic 20 ms Opus frame (zeroed; real usage: encode from PCM).
        opus = bytes(40)

        for i in range(3):
            frame = gap_alice.send(
                alice, alice_mls,
                target=2,
                media_source_id=1,
                rtp_timestamp=i * 960,
                opus=opus,
                codec=PayloadCodec.FLATBUFFERS,  # lowest decode latency
            )
            for ev in bob.on_wire(bob_mls, frame.wire):
                if ev.kind == "payload_received" and ev.stream_type == 1:
                    r = gap_bob.accept(ev.plaintext or b"", bob_mls.epoch,
                                      codec=ev.codec or PayloadCodec.CBOR)
                    print(f"frame {i+1}: status={r.status}  seq={r.seq}  codec={ev.codec}")
