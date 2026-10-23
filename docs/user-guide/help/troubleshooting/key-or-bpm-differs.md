# My key or BPM differs from another app

<p class="lead">Another app shows a different key or BPM for the same track. In most cases the two numbers are not measuring the same thing, or are written differently. This page helps you tell which, and what to do about each case, in about ten minutes.</p>

## Why it happens { #why }

CueRithm measures key, BPM and the beatgrid from the decoded audio, on your computer, and never from tags or file names. Another app may disagree for one of seven reasons:

| Reason | What it looks like |
|---|---|
| **Different notation** | CueRithm shows the key as a Camelot code. The demo library's *Come To Life (Original Mix)* is <span class="cr-key">7A</span>, which is D minor. An app set to musical names or Open Key shows the same key in other letters. |
| **The other app reads a tag, not the audio** | A file can carry a key and BPM tag written by Beatport, Mixed In Key or another tool. CueRithm's **KEY** and **BPM** columns always show its own measurement, and the library keeps it even when the file's tag says something else. |
| **Half or double tempo** | CueRithm searches 98 to 195 BPM and folds every track into that range. A 94 BPM track is reported as 188. A 140 BPM dubstep track such as *Play With Fire (Original Mix)* can be counted as 70 by another tool. There is no setting for the range. |
| **A constant grid** | CueRithm saves one BPM and one first beat per track. A live drummer or a tempo change drifts away from the beat lines, and another app may place its first beat or its bar start somewhere else. |
| **Rounding** | The table shows BPM to two decimals (130.00). Other apps may round to one decimal or to a whole number. |
| **The key is a prediction** | The key comes from a neural network. Close disagreements are common: a neighbouring key on the Camelot wheel, or the relative major or minor. |
| **Very short or silent audio** | Under about 10 seconds there is no BPM or grid, and under about 3 seconds there is no key. The track then stays in Incoming. See [A track is stuck in Incoming](incoming-stuck.md). |

CueRithm's agreement with another tool is measured, not guaranteed. On the owner's 282 tuning tracks, 274 (97.16%) had the same key as the stored labels of one reference tool, and held-out agreement has not been measured. [How accurate is it?](../../how-it-works/accuracy.md) gives the evidence and its limits.

## Fix it { #fix }

1. **Compare like with like.** Convert the other app's key to Camelot, or yours to its notation. [Understand Camelot keys](../../analysis/camelot-keys.md) has the conversions.
2. **Find out what the other app reads.** Select the track and choose **Edit metadata**. The **Measured analysis** block (read-only) shows CueRithm's key and BPM. Under **File key and BPM overrides**, **File key tag** and **File BPM tag** show what is written in the file. If the other app agrees with the tags, the audio measurement is not the cause. [Edit tags and artwork](../../tagging/edit-tags-by-hand.md) explains the editor.
3. **Choose what CueRithm writes to files.** In **Settings** → **Automatic tagging** → **Fields to write**, **Key and BPM written to files** is **Cuerithm analysis** or **Catalog metadata**. **Write Camelot keys** chooses the notation. Changing the setting affects tracks prepared after you save; use **Analyze again** for finished tracks ([Analyze a track again](../../analysis/analyze-again.md)).
4. **Check by ear.** Play the track in the 32-second view and watch whether the beat lines land on the kicks ([Check the beatgrid at beat level](../../analysis/check-the-beatgrid.md)). Trust your ears over either number.
5. **Correct a grid.** Open the track in the player and choose **Edit cues & grid**. The **Constant beatgrid** form has **Grid BPM**, **First beat seconds** and the buttons **Half BPM** (½) and **Double BPM**. Preview the correction, then save it ([Fix a wrong beatgrid](../../analysis/corrections/fix-the-beatgrid.md)). The player, the beat lines and Sync use the corrected grid. The measured detector result stays unchanged.
6. **Sending to rekordbox?** If rekordbox already has the grid you want, choose **Keep rekordbox** for that field in the sync review ([Resolve rekordbox sync conflicts](../../sync/rekordbox-conflicts.md)).

CueRithm 0.1.13 has no manual correction for the key. You can change the file's key tag in **Edit metadata**, but the measured key in the library does not change.

## Check it worked { #check }

- The beat lines in the player sit on the beats for the whole 32-second window, at the start, middle and end of the track.
- The BPM you read in the player matches what you hear.
- The other app and CueRithm agree once both use the same notation, or you have decided which tag your DJ software should read.

## Still stuck? { #still-stuck }

If a track is clearly wrong and you have ruled out notation, tags and tempo folding, [report it](../report-a-problem.md) with the file type, CueRithm's key and BPM, and what the other tool shows. Do not attach copyrighted audio.

## Related

- [Read a track's key, BPM and energy](../../analysis/read-key-bpm-energy.md)
- [How is the key detected?](../../how-it-works/key.md)
- [How are BPM and the beatgrid found?](../../how-it-works/tempo.md)
- [Fix a wrong beatgrid](../../analysis/corrections/fix-the-beatgrid.md)

<!-- verified: crates/camelot-core/src/key.rs (Camelot mapping, D minor = 7A), openkeyscan.rs (3 s guard), lexicon_timing/mod.rs (98-195 BPM defaults, 10 s minimum); apps/desktop/src/components/TrackTable.tsx (BPM toFixed(2), Key chip); MetadataEditor.tsx (Measured analysis read-only), MetadataFields.tsx (File key and BPM overrides, File key tag, File BPM tag); TaggingSettings.tsx (Key and BPM written to files: Cuerithm analysis/Catalog metadata, Write Camelot keys); PlayerPanel.tsx (Edit cues & grid); PublicationEditor.tsx (Constant beatgrid, Grid BPM, First beat seconds, Half BPM, Double BPM, Preview corrections, Save preview); published-analysis.ts (player uses published grid); RekordboxConflicts.tsx (Keep rekordbox); docs/research/engine-benchmarks.md (274/282, 97.16%); legacy technical/accuracy.md, key.md, tempo.md re-checked -->
