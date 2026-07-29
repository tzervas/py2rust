# py2rust — high priority

- root: `/workspace/repos/tg-agent-relay`
- budget: 15 item(s); full detail in the corpus report

## Signatures only — most unlowered body statements

- `tg_agent_relay/poll.py` — id `body:565628a1` — 577 of 618 statements unlowered
- `lib/dashboard_render.py` — id `body:ac6e10f5` — 512 of 538 statements unlowered
- `tg_agent_relay/send.py` — id `body:8b654cf4` — 491 of 524 statements unlowered
- `lib/usage_ingest.py` — id `body:d7a912da` — 474 of 504 statements unlowered
- `tg_agent_relay/format_api.py` — id `body:8548f09a` — 328 of 345 statements unlowered
- `tests/test_poll.py` — id `body:54a810f2` — 319 of 331 statements unlowered
- `tg_agent_relay/threads.py` — id `body:e57b9bef` — 286 of 313 statements unlowered
- `lib/routing.py` — id `body:65464316` — 274 of 296 statements unlowered
- `lib/remote_config.py` — id `body:88cb74db` — 267 of 292 statements unlowered
- `tests/test_usage_ingest.py` — id `body:0609f1f9` — 257 of 261 statements unlowered
- `tests/test_send.py` — id `body:e9b7b2d7` — 227 of 236 statements unlowered
- `providers/claude/hooks.py` — id `body:048c2fb0` — 203 of 211 statements unlowered
- `tg_agent_relay/mcp_stub.py` — id `body:3819eef1` — 198 of 209 statements unlowered
- `tests/test_grok_adapter_e2e.py` — id `body:e0ba4187` — 191 of 206 statements unlowered
- `lib/fifo_agent_readers.py` — id `body:750438fb` — 179 of 192 statements unlowered

## Largest gap categories

Ranked by **frequency**, which is a proxy. Frequency says what is common;
it does not say what is leveraged. The ranking that matters is blast
radius — how many other gaps each root cause unblocks — and it arrives
once gaps record their dependencies.

- `DynamicTyping` — 753
- `FunctionBody` — 584
- `Other` — 502
- `Import` — 467
- `Exception` — 227

## Omitted

Showing **15** of **91** candidate item(s); **76** not listed.
Total recorded gaps across the corpus: **2788**.

This is a worklist, not a survey. Absence from it is not evidence of
correctness — see the corpus report for everything.
