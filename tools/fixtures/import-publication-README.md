# Import file-publication controls

`import-publication-media.tar.gz` contains only an invented one-second 440Hz
mono signal, transcoded into the ten required containers/codecs. The signal and
fixtures are CC0. Its manifest records each file SHA-256, encoder and generation
version. No purchased music, source-app extracts, accounts or reference labels
are included.

Regenerate with `python3 tools/fixtures/generate_import_publication.py` using a
locally installed encoder-enabled FFmpeg. This is a developer fixture tool.
Tests read the committed archive and the application's sealed decoder runtime;
the installed product and tests require neither this Python tool nor an encoder.
The fixture establishes bounded file-tag writing, PCM preservation and exact-byte
Undo in controlled files. It establishes no musical-prediction accuracy or native
DJ-app acceptance.
