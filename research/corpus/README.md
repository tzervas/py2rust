# Corpus batch snapshots (tg-agent-relay)

Measured with `py2rust batch` against `tzervas/tg-agent-relay` (91 modules).

| snapshot | gaps | L1 | L2 | L3 no-stub | Import | notes |
|---|---:|---:|---:|---:|---:|---|
| `tg-agent-relay-20260729` | 2865 | 35.5% | 5.3% | 14.3% | 544 | post Wave A (type-map, multi-stmt, erase typing) |
| `tg-agent-relay-20260729-waveb` | 2788 | 39.4% | 5.3% | 14.3% | 467 | + for/range + erase collections.abc/pathlib |

## Ranked categories (Wave B tip)

1. DynamicTyping  
2. FunctionBody  
3. Other (module-level expr, with, nested fns)  
4. Import (stdlib runtime still open)  
5. Exception  
6. Comprehension  

Steer by **L2** and **L3 no-stub**, not L1.
