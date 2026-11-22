# Mini 4.8 — Checked indexing (development release)

Mini 4.8 supports variable indices into **small fixed-size arrays**.

```mini
program {
    let data: [usize; 4] = [0; 4]
    let i: usize = 0
    while i < 4 {
        data[i] = i + 39
        i = i + 1
    }
    return data[3] as i64
}
```

**Provable at compile time:** `data[1 + 2]` directly selects element 3.
An out-of-range constant such as `data[2 + 2]` is a compile error.

**Not provable:** `data[i]` evaluates the index exactly once, then checks a
bounded chain of comparisons for indices `0..N-1`. If no index matches, execution
halts in a trap loop **without reading or writing out-of-bounds memory**.

## Limitations

- Dynamic indexing is capped at **16 elements**. Constant-index arrays retain
  the previous maximum of 1024 elements.
- Dynamic index expressions must have type `usize`, without implicit signed casts.
- Access uses an **O(N) branch chain**, not efficient O(1) indexed addressing.
- `while data[i] == ...` is rejected because the condition needs careful
  re-evaluation on each iteration. Accesses in the loop body are supported.
- **Loop range analysis is not yet implemented.** Dynamic checks are not yet
  eliminated merely because a loop seems to constrain its index.
- Invalid runtime indices halt in a loop; there is no recoverable error value.
- Embedded code size and timing, Cargo build, and native firmware execution
  still require verification on actual targets.

A future release should add compact contiguous layouts, O(1) dynamic access,
proved loop bounds, and removable checks. Linux dynamic memory management
continues to belong in included Mini libraries.
