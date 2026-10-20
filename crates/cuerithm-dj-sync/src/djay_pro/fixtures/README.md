# Synthetic djay object fixture

`native-user-data.bin` is a 763-byte current-format TSAF object generated from invented metadata through an observed local reference serializer. It contains no application code, music, library paths, credentials, or real collection metadata.

The fixture covers native string pointers and aligned numeric values with the UUID `synthetic-native-reference`, title `Café`, artist `Synthetic`, duration 300 seconds, BPM 120, rating 4, manual gain -2.5, key index 1, a cue at 1.5 seconds, a loop at 20–24 seconds, and a grid anchor at 0.25 seconds. The reference implementation was Lexicon 1.11.15's current djay object writer. The Rust test decodes its values and verifies independent reserialization.
