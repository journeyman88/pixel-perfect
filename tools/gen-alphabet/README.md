# gen-alphabet

Generates [`src/import/alphabet/table.rs`](../../src/import/alphabet/table.rs),
the alphabet `pixel-perfect import` draws palette characters from.

The alphabet holds every character the schema's `char` pattern
(`^[^.=\s\p{Cc}\p{Cf}\p{M}]$`) accepts, in two tiers:

- **Tier 1**: single-column, visible characters. These are assigned, not
  private use, not East Asian Wide or Fullwidth, not right-to-left (bidi
  `R`/`AL`), not `Default_Ignorable_Code_Point` and not noncharacters. They
  start with a fixed ASCII prefix (`a`–`z`, `A`–`Z`, `0`–`9`,
  `!#$%&()*+,-/:;<>?@[]^_{|}~`, then `"`, `'`, `\` and the backtick) and
  continue in code point order.
- **Tier 2**: every other accepted character, in code point order.

The Unicode version must match the one used by `regex-syntax`, the crate
`jsonschema` uses to evaluate the pattern. Otherwise the alphabet and the
schema disagree, and the alphabet tests in `src/import/alphabet.rs` fail.
`regex-syntax` names its version at the top of
`src/unicode_tables/general_category.rs`.

## Regenerating

```sh
V=16.0.0
mkdir -p /tmp/ucd-$V && cd /tmp/ucd-$V
for f in UnicodeData.txt EastAsianWidth.txt DerivedCoreProperties.txt \
         PropList.txt extracted/DerivedBidiClass.txt; do
  curl -sSfLO https://www.unicode.org/Public/$V/ucd/$f
done
cd -   # back to the repository root
cargo run --release --manifest-path tools/gen-alphabet/Cargo.toml -- \
  /tmp/ucd-$V $V > src/import/alphabet/table.rs
```

Changing the table changes which characters an import assigns, so it is a
specification change (see `openspec/specs/png-import/spec.md`).
