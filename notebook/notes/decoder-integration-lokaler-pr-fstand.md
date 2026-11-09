---
id: a_773e9313733a
title: "Decoder-Integration: lokaler Prüfstand"
type: markdown
tags: []
owner: {"kind":"agent","agentId":"codex-cli","name":"kroko-stt-german-wyoming","workspace":"kroko-stt-german-wyoming"}
createdAt: 2026-11-09T07:24:11Z
updatedAt: 2026-11-09T07:24:11Z
pinned: false
---
# Decoder-Integration in Kroko STT Wyoming

Die zentrale Decoderwahl verwendet für das offizielle deutsche Community-128 exakt modified_beam_search, max_active_paths=4 und blank_penalty=0. Classic, Community-64, andere Sprachen und Custom-Modelle behalten Greedy. Kein ENV-Override und keine Beam-8-Option.

68 Rust-Tests und sieben Template-Tests erfolgreich. Classic-/Community-64-/Custom-Wyoming-Smokes bestanden. Die Classic-/Community-64-Texte stimmen mit historischen Resultaten überein.

100 deutsche FLEURS-Anfragen über den isolierten Rust-/Wyoming-Pfad ergeben WER 6,305 % und CER 4,291 %. Alle 100 normalisierten Texte stimmen mit dem bestätigten Beam-4-Benchmark überein. Die alte Greedy-Baseline wurde ebenfalls mit 100 identischen Texten reproduziert.

Vorher / nachher: Median RTF 0,1189 / 0,1212, P95 RTF 0,1850 / 0,1934; CPU-Zeit +5,02 %; Peak RAM 301,9 / 308,0 MiB. Fünf gepacete Anfragen je Variante: Ende-Audio bis Text Median 224,1 / 234,0 ms. Interne fortlaufende Partials vor Audioende nachgewiesen; Wyoming liefert weiterhin finale Texte. Neustart mit vollständigem read-only-Modell erfolgreich.

Stream-Lebenszyklus, PCM-Verarbeitung, Tail, Endpointing und Wyoming-Protokollcode sind unverändert. Originalmodelle, Downloadlogik und produktive Container wurden nicht verändert. Kein neues Image, kein Push, Tag oder Release. Der freigegebene nächste Schritt wäre die getrennte Veröffentlichung; dieser Prüfstand wird nur lokal committed.
