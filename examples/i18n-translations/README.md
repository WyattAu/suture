# Translation File Management

Merge translation files from multiple translators without conflicts.

## Problem

Localization teams often have translators working on different languages in
parallel. When multiple translators add new translation keys to their
respective locale files, a traditional VCS treats these as text conflicts —
even though each translator touches a completely different file, or different
keys within the same file.

## Solution

Suture understands JSON structure. When translators add new keys to locale
files, Suture merges them semantically. New keys from all translators are
preserved, and no existing translations are lost — regardless of the order
they arrive.

## What This Example Demonstrates

- Committing base English, Spanish, and Japanese locale files
- A Spanish translator adding new keys on `translator-es`
- A Japanese translator adding new keys on `translator-ja`
- Merging both branches — all new keys preserved, no conflicts
- Verifying no translations were lost

## Running

```bash
make
```

## Expected Outcome

Both translator branches merge cleanly. The final locale files contain all
original translations plus the new keys added by each translator.
