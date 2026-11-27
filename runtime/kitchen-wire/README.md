# kitchen-wire

The runtime API on the wire ([standard §6](../../standard/spec/06-runtime-api.md), decision 0017):

- `pb`: the messages of
  [`contract/proto/kitchen/runtime/v1/runtime.proto`](../../contract/proto/kitchen/runtime/v1/runtime.proto),
  compiled with prost and protox (no `protoc` needed);
- conversions to and from `kitchen-client`'s calls (`encode_request`, `decode_answer`, …), with
  documents (ops, pipelines, records) carried as their JSON inside;
- framing: a 4-byte big-endian length, bounded before allocating;
- `chunk`: data as raw arrays (dtype, shape, region, level) or Arrow IPC tables, lz4 or zstd.

`tests/breaking.rs` compares the protocol with the committed `runtime.lock.json`: fields may be
added (`UPDATE_WIRE_LOCK=1` records them), never renumbered, retyped or removed.
