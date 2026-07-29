# py2rust corpus report

- root: `/workspace/repos/tg-agent-relay`
- files: 91 discovered, 91 parsed, 0 failed
- top-level items: 1987 | emitted: 705 | gaps: 2865
- L1 top-level expressible: 35.5%
- **L2 statement coverage: 5.3%** (585 of 10955 statements lowered)
- statements inside bodies: 8968 (81.9% of all statements)

> **L2 is the number to steer by.** L1 counts only top-level items, so it
> measures against 1987 statements while the module actually contains 10955.
> L2 counts every statement, nested included, and an emitted signature
> whose body did not lower counts as exactly one statement — not as its
> whole body.

- L3 compiles: 100.0% (91 of 91 emitted modules accepted by `rustc`)
- **L3 compiles with no stub body: 14.3%** (13 of 91 modules)
- unlowered bodies across the corpus: 584

> Note: 78 of the 91 compiling module(s) still contain `todo!()` bodies,
> so they build but panic when called. Only the second figure is progress.

> Note: 584 of 705 emitted items (83%) are signature-only
> (`FunctionBody` gap). The rest lowered fully.

## Gap categories by measured frequency

| rank | category | count | share |
|---|---|---:|---:|
| 1 | `DynamicTyping` | 753 | 26.3% |
| 2 | `FunctionBody` | 584 | 20.4% |
| 3 | `Import` | 544 | 19.0% |
| 4 | `Other` | 502 | 17.5% |
| 5 | `Exception` | 227 | 7.9% |
| 6 | `Comprehension` | 135 | 4.7% |
| 7 | `MultiStmtBody` | 61 | 2.1% |
| 8 | `Class` | 31 | 1.1% |
| 9 | `Lambda` | 22 | 0.8% |
| 10 | `Metaprogramming` | 6 | 0.2% |

## Modules by expressible fraction

Highest first — cheapest to finish by hand.

| module | top-level | emitted | gaps | expressible | dominant gaps |
|---|---:|---:|---:|---:|---|
| `lib/routing.py` | 27 | 21 | 58 | 76% | DynamicTyping×21, FunctionBody×19, Import×8 |
| `tests/test_agent_handle.py` | 9 | 7 | 10 | 75% | FunctionBody×6, Import×3, Other×1 |
| `tests/test_comms_format.py` | 8 | 6 | 8 | 71% | FunctionBody×5, DynamicTyping×1, Import×1 |
| `lib/metrics_agg.py` | 17 | 12 | 32 | 67% | FunctionBody×9, DynamicTyping×7, Comprehension×4 |
| `tests/test_media_inbound.py` | 10 | 7 | 10 | 67% | FunctionBody×6, Import×2, Other×2 |
| `tg_agent_relay/bots.py` | 21 | 14 | 32 | 63% | DynamicTyping×15, FunctionBody×12, Comprehension×2 |
| `tg_agent_relay/plan_approve.py` | 29 | 19 | 43 | 63% | DynamicTyping×17, FunctionBody×17, Import×6 |
| `tg_agent_relay/threads.py` | 39 | 25 | 78 | 62% | DynamicTyping×27, FunctionBody×23, Import×11 |
| `lib/usage_ingest.py` | 44 | 28 | 91 | 62% | FunctionBody×26, DynamicTyping×21, Exception×20 |
| `lib/fifo_agent_readers.py` | 19 | 12 | 48 | 61% | Exception×16, FunctionBody×11, DynamicTyping×10 |
| `tests/test_goal_events.py` | 6 | 4 | 5 | 60% | FunctionBody×3, Import×1, Other×1 |
| `tests/test_remote_config.py` | 16 | 10 | 16 | 60% | FunctionBody×9, Import×3, Exception×1 |
| `tg_agent_relay/config.py` | 12 | 8 | 32 | 60% | DynamicTyping×11, FunctionBody×6, Import×6 |
| `lib/remote_config.py` | 39 | 24 | 62 | 59% | FunctionBody×22, DynamicTyping×18, Exception×10 |
| `tests/test_threads.py` | 18 | 11 | 30 | 59% | FunctionBody×10, DynamicTyping×9, Import×5 |
| `tg_agent_relay/agent_handle.py` | 21 | 13 | 24 | 58% | FunctionBody×11, Import×5, DynamicTyping×4 |
| `tg_agent_relay/format_api.py` | 28 | 17 | 40 | 58% | FunctionBody×15, DynamicTyping×11, Import×6 |
| `tests/test_agent_stamp.py` | 8 | 5 | 7 | 57% | FunctionBody×4, Import×2, Other×1 |
| `tg_agent_relay/poll.py` | 69 | 39 | 144 | 55% | DynamicTyping×61, FunctionBody×37, Import×25 |
| `tg_agent_relay/send.py` | 55 | 31 | 95 | 55% | FunctionBody×29, DynamicTyping×24, Import×21 |
| `lib/dashboard_render.py` | 45 | 25 | 139 | 55% | DynamicTyping×38, Comprehension×34, FunctionBody×24 |
| `tg_agent_relay/extensions.py` | 25 | 14 | 47 | 52% | DynamicTyping×19, FunctionBody×12, Import×5 |
| `lib/code_highlight.py` | 11 | 6 | 26 | 50% | Import×10, Exception×5, FunctionBody×5 |
| `lib/sessions.py` | 24 | 13 | 46 | 50% | DynamicTyping×21, FunctionBody×11, Import×7 |
| `providers/claude/usage.py` | 8 | 5 | 14 | 50% | Exception×4, Import×4, FunctionBody×3 |
| `providers/grok/usage.py` | 10 | 6 | 26 | 50% | Exception×9, Import×7, FunctionBody×4 |
| `tests/test_plan_approve.py` | 7 | 4 | 6 | 50% | FunctionBody×3, Import×2, Other×1 |
| `tg_agent_relay/spool.py` | 25 | 13 | 48 | 50% | DynamicTyping×13, Exception×12, FunctionBody×12 |
| `tg_agent_relay/cli.py` | 12 | 6 | 20 | 45% | Import×11, FunctionBody×5, Exception×2 |
| `tg_agent_relay/goal_events.py` | 13 | 7 | 12 | 45% | DynamicTyping×5, FunctionBody×5, Import×1 |
| `tests/test_sessions_routing.py` | 15 | 7 | 18 | 43% | Import×9, FunctionBody×6, DynamicTyping×1 |
| `tg_agent_relay/agent_stamp.py` | 22 | 10 | 30 | 43% | FunctionBody×9, Import×9, DynamicTyping×6 |
| `lib/provider_hook.py` | 13 | 6 | 24 | 42% | Import×8, DynamicTyping×5, FunctionBody×5 |
| `tests/test_spool.py` | 18 | 8 | 25 | 41% | FunctionBody×7, Import×5, DynamicTyping×4 |
| `providers/claude/hooks.py` | 17 | 8 | 26 | 40% | DynamicTyping×6, FunctionBody×6, Other×5 |
| `providers/grok/hooks.py` | 20 | 9 | 29 | 39% | DynamicTyping×8, FunctionBody×7, Other×6 |
| `tests/test_session_handler.py` | 19 | 8 | 20 | 39% | Import×8, FunctionBody×7, DynamicTyping×2 |
| `tg_agent_relay/media_inbound.py` | 33 | 14 | 45 | 39% | DynamicTyping×16, FunctionBody×12, Import×10 |
| `tests/test_grok_adapter_e2e.py` | 35 | 14 | 43 | 38% | FunctionBody×13, DynamicTyping×10, Import×10 |
| `providers/base.py` | 23 | 10 | 32 | 38% | DynamicTyping×11, FunctionBody×8, Class×4 |
| `tg_agent_relay/adk_bridge.py` | 16 | 7 | 26 | 36% | Import×8, DynamicTyping×5, FunctionBody×5 |
| `tg_agent_relay/mcp_stub.py` | 25 | 10 | 38 | 35% | DynamicTyping×13, Import×9, FunctionBody×8 |
| `lib/context_select.py` | 10 | 4 | 15 | 33% | Import×4, DynamicTyping×3, FunctionBody×3 |
| `providers/ollama/usage.py` | 8 | 4 | 8 | 33% | DynamicTyping×2, FunctionBody×2, Class×1 |
| `providers/openai/usage.py` | 8 | 4 | 8 | 33% | DynamicTyping×2, FunctionBody×2, Class×1 |
| `tests/test_bots.py` | 19 | 7 | 25 | 33% | Import×7, FunctionBody×6, DynamicTyping×5 |
| `tests/test_fifo_agent_readers.py` | 16 | 6 | 21 | 33% | Import×8, FunctionBody×5, Other×4 |
| `tests/test_highlight_docs.py` | 16 | 6 | 19 | 33% | DynamicTyping×5, FunctionBody×5, Import×4 |
| `tg_agent_relay/metrics.py` | 7 | 3 | 9 | 33% | Import×3, DynamicTyping×2, FunctionBody×2 |
| `tg_agent_relay/tts.py` | 23 | 9 | 38 | 33% | DynamicTyping×11, Exception×8, FunctionBody×7 |
| `tests/test_context_select.py` | 17 | 6 | 23 | 31% | Import×6, FunctionBody×5, Comprehension×4 |
| `tests/test_usage_allotments.py` | 11 | 4 | 10 | 30% | FunctionBody×3, Import×3, DynamicTyping×2 |
| `tests/test_extensions_adk.py` | 18 | 6 | 24 | 29% | Import×8, DynamicTyping×5, FunctionBody×5 |
| `tests/test_providers_plugplay.py` | 18 | 6 | 25 | 29% | Import×7, DynamicTyping×5, FunctionBody×5 |
| `tg_agent_relay/hooks.py` | 9 | 4 | 12 | 29% | Import×6, DynamicTyping×2, FunctionBody×2 |
| `lib/tts_plain_text.py` | 21 | 6 | 32 | 25% | DynamicTyping×9, Other×8, FunctionBody×5 |
| `tests/test_format_api.py` | 9 | 3 | 20 | 25% | Other×6, Import×5, Comprehension×4 |
| `tests/test_metrics_agg.py` | 9 | 3 | 16 | 25% | Import×7, Other×5, FunctionBody×2 |
| `tests/test_package_interfaces.py` | 9 | 3 | 15 | 25% | Import×5, Other×5, FunctionBody×2 |
| `tests/test_tts_plain_text.py` | 9 | 3 | 11 | 25% | Import×4, Other×3, FunctionBody×2 |
| `tests/test_usage_ingest.py` | 9 | 3 | 19 | 25% | Import×11, Other×4, FunctionBody×2 |
| `tg_agent_relay/highlight_docs.py` | 23 | 7 | 34 | 24% | Import×12, DynamicTyping×10, FunctionBody×5 |
| `tests/test_code_highlight.py` | 10 | 3 | 23 | 22% | Comprehension×7, Import×6, Other×5 |
| `tests/test_providers_claude.py` | 10 | 3 | 20 | 22% | Import×7, Comprehension×4, Other×4 |
| `tg_agent_relay/comms_format.py` | 25 | 6 | 30 | 21% | DynamicTyping×11, Import×7, FunctionBody×5 |
| `lib/context_render.py` | 16 | 4 | 23 | 20% | Import×8, DynamicTyping×7, FunctionBody×3 |
| `lib/toml_to_json.py` | 5 | 1 | 8 | 20% | Import×3, Exception×2, FunctionBody×1 |
| `tests/test_hook_fixtures.py` | 26 | 6 | 31 | 20% | DynamicTyping×9, Import×7, FunctionBody×5 |
| `tests/test_project_bind.py` | 21 | 5 | 23 | 20% | Other×7, Import×6, DynamicTyping×5 |
| `tests/test_providers_grok.py` | 11 | 3 | 22 | 20% | Import×7, Comprehension×5, Other×5 |
| `tests/test_usage_registry.py` | 11 | 3 | 22 | 20% | Import×8, Other×5, Comprehension×3 |
| `tg_agent_relay/routing.py` | 23 | 6 | 25 | 19% | DynamicTyping×12, FunctionBody×4, Import×4 |
| `tests/test_routing_tables.py` | 12 | 3 | 17 | 18% | Import×8, Other×4, FunctionBody×2 |
| `lib/provider_catalog.py` | 8 | 2 | 15 | 14% | Comprehension×6, Import×6, FunctionBody×1 |
| `tests/test_send.py` | 79 | 8 | 87 | 9% | Other×49, DynamicTyping×19, Import×10 |
| `tests/test_mcp_stub.py` | 67 | 5 | 68 | 6% | Other×31, DynamicTyping×23, Import×7 |
| `tests/test_poll.py` | 273 | 11 | 287 | 4% | Other×150, DynamicTyping×97, Import×15 |
| `tests/test_tts_package.py` | 102 | 2 | 101 | 1% | Other×69, DynamicTyping×25, Import×3 |
| `providers/__init__.py` | 13 | 1 | 12 | 0% | Import×9, Other×2, DynamicTyping×1 |
| `providers/adk/__init__.py` | 6 | 1 | 5 | 0% | DynamicTyping×2, Exception×1, Import×1 |
| `providers/aider/__init__.py` | 5 | 1 | 4 | 0% | DynamicTyping×2, Import×1, Other×1 |
| `providers/claude/__init__.py` | 6 | 1 | 5 | 0% | Import×3, DynamicTyping×1, Other×1 |
| `providers/gemini/__init__.py` | 5 | 1 | 4 | 0% | DynamicTyping×2, Import×1, Other×1 |
| `providers/generic/__init__.py` | 4 | 1 | 3 | 0% | DynamicTyping×1, Import×1, Other×1 |
| `providers/grok/__init__.py` | 6 | 1 | 5 | 0% | Import×3, DynamicTyping×1, Other×1 |
| `providers/ollama/__init__.py` | 6 | 1 | 5 | 0% | DynamicTyping×2, Import×2, Other×1 |
| `providers/openai/__init__.py` | 7 | 1 | 6 | 0% | DynamicTyping×3, Import×2, Other×1 |
| `tests/conftest.py` | 2 | 1 | 1 | 0% | Other×1 |
| `tests/test_script_suites_pytest.py` | 10 | 1 | 9 | 0% | Import×4, DynamicTyping×3, Metaprogramming×1 |
| `tg_agent_relay/__init__.py` | 3 | 1 | 2 | 0% | DynamicTyping×1, Other×1 |
| `tg_agent_relay/protocols.py` | 15 | 2 | 13 | 0% | Class×8, Import×3, DynamicTyping×1 |

## L3 — emitted Rust that `rustc` rejects

None — all 91 compiled module(s) were accepted. See the stub-body count
above before reading that as working code.

## Files that did not parse

None.

