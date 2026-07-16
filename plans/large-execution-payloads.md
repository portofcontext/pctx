# Large execution payloads over WebSocket

## Problem
`execute_code` responses are serialized to a **single WebSocket text frame** ([handler.rs](../crates/pctx_session_server/src/websocket/handler.rs)). That frame is bounded by tungstenite's **default 16 MiB `max_frame_size`** — enforced by the *receiver* (the client), not configured anywhere in this repo. Two ways to exceed it:

- **Trace bloat:** every tool call's full output is cloned into `trace.events[].outcome.output` ([registry.rs:365](../crates/pctx_registry/src/registry.rs#L365)). A script that returns a tiny value but calls a tool with large results still produces a >16 MiB trace, and the whole response fails to send.
- **Large return value / stdout:** the script itself returns or prints something huge.

Either way the response silently fails to reach the client.

## Current short-term solution (shipped)
[`truncate::bound_response_size`](../crates/pctx_session_server/src/websocket/truncate.rs) shrinks an oversized response before it's sent, under a 15 MiB ceiling (`MAX_RESPONSE_BYTES`, margin under the 16 MiB frame limit):
1. Drop the trace (the usual culprit).
2. If still too large, replace the return value with a `{ "__truncated__": true, "reason": … }` marker and append the reason to `stderr`, so the calling agent knows data was dropped and why.

This keeps every response deliverable but is **lossy** — the trace and large return values are discarded, not retrievable. The long-term goal is to deliver them without breaking the frame limit.

---

## Long-term options

### A WebSocket fact worth knowing
A WS *message* is already split into *frames* by the sender and reassembled by the receiver up to `max_message_size` (default **64 MiB**). The 16 MiB we hit is the per-*frame* cap; message cap is 64 MiB — both are just config. So "manual chunking" mostly reinvents what the protocol already does.

### Two axes
- **Delivery** — how the client gets the bytes: inline (bigger frames / compression) vs. **out-of-band** (return a reference/URL, client pulls).
- **Storage** — where oversized bytes live: server memory, the session backend, or object storage.

### Delivery options

| Option | How | Pros | Cons |
|---|---|---|---|
| **A. Raise frame/message limits** | Set `WebSocketConfig` on `on_upgrade` ([handler.rs](../crates/pctx_session_server/src/websocket/handler.rs)) | ~1 line; buys headroom | Moves the ceiling, doesn't remove it; big frames = memory spikes + head-of-line blocking; **every client must raise its receive cap too** |
| **B. Compress** | permessage-deflate WS extension, or app-level gzip | JSON compresses ~5-10×; transparent | Still bounded; CPU cost; no help for truly huge data |
| **C. Out-of-band reference + fetch** | On overflow, persist the payload, return a small `{ ref }`; client fetches it separately (REST endpoint **or** presigned URL) | Hot path stays tiny; client pulls only when needed; scales to any size; covers `output` + trace | New endpoint/URL + client change; needs a storage layer + retention/GC; extra round-trip |
| **D. Manual chunk + reassemble** | Split into N WS messages the client stitches | Stays on one channel | Reinvents WS framing; ordering/reassembly bugs — **not recommended** |

### Storage layer for C
`post_execution` ([handler.rs](../crates/pctx_session_server/src/websocket/handler.rs)) is only a hook — the deployed backend is **Redis** ([state/mod.rs:37](../crates/pctx_session_server/src/state/mod.rs#L37)), which is the wrong home for multi-MB blobs (memory pressure, value-size limits, eviction). C's reference indirection is cheap, but the *bytes* belong in a persistent blob layer:
- **Object storage (S3/GCS)** — standard home; the reference is then a **presigned URL** (client downloads directly, server never re-streams) or an opaque id the server resolves.
- A dedicated blob table/store if keeping it in-house.

### Also worth considering
- **Cap at the source.** Truncate/summarize each tool output *before* it enters the trace ([registry.rs:365](../crates/pctx_registry/src/registry.rs#L365)) — e.g. first N KB + `original_bytes`. Prevents bloat instead of transporting it. Cheapest durable fix; pairs with anything.
- **Pull-based pagination.** Trace becomes N events fetched on demand (`GET /executions/{id}/trace?offset=`). Natural extension of C.
- **Content-addressed dedup.** Store a recurring large blob once by hash, reference by id.

---

## Recommendation
Cheapest-first, layered:
1. **Ship the short-term bound** — done.
2. **Cap tool outputs at the source** — kills the common cause with minimal surface.
3. **Option C (out-of-band reference + fetch), backed by object storage** (not Redis) — return a presigned download URL for large payloads; covers oversized `output`/`stdout` as well as trace. Add **B (compression)** as a transparent multiplier for mid-size payloads.

Skip **D** (manual chunking). Treat **A** (raising limits) as a temporary knob only, and remember every client must raise its cap too.
