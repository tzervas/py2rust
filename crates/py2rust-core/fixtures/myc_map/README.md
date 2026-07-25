# myc_map fixtures

Owned by the **surface-mapper** lane (`crates/py2rust-core/src/myc_map.rs`).

These notes document the Mycelium-native spellings used in the mapping table.
They are **not** executable `.myc` programs in this repo (py2rust does not vendor
the Mycelium toolchain). Authority lives on each `MapOutcome` row as a
`Citation` (Corpus / Test / Unverified).

| PyConstruct   | Mycelium-native direction                                      | Status      |
|---------------|----------------------------------------------------------------|-------------|
| Lambda        | `lambda(x: T) => expr`                                         | Mapped      |
| MultiStmtBody | nested `let x = a in (let y = b in …)` (block sugar separate)  | Mapped      |
| Exception     | `Result[A,E]` + `std.recover`                                  | Mapped      |
| Import        | `nodule …` / confirmed nodule path only                        | Mapped      |
| Comprehension | `std.iter` / recursive fn (no list-comp sugar claimed)         | Mapped*     |
| PartialEmit   | `fn … = <body-or-gap>` never-silent FunctionBody               | Mapped      |
| Class         | ADT / impl — no OO class                                       | Unmappable  |
| DynamicTyping | static surface only                                            | Unmappable  |
| Metaprogramming | no eval/metaclass surface                                    | Unmappable  |
| Async         | no async-def → spore policy yet                                | Unmappable  |
| Other         | explicit catch-all refusal                                     | Unmappable  |

\*Comprehension carries `Citation::Unverified` for the exact combinator shapes.
