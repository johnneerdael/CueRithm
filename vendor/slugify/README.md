# slugify transliteration data

`char-map.json` is the default character map from MIT-licensed slugify 1.6.6:
https://github.com/simov/slugify/tree/v1.6.6

The upstream source was downloaded from that tag and its character-map JSON was
decoded and formatted without changing entries. `UPSTREAM.json` records hashes
of the downloaded source, retained license and derived map. The JavaScript source
is not shipped or executed by the Rust application. No locale-specific map is
selected by the inspected reference helper.

The independent Rust matcher applies the strict transliteration options needed
for duplicate normalization. Retain `LICENSE` when distributing the data.
