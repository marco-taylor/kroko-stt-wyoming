# Kroko Classic: Streaming- und Decoding-Benchmark

**Empfehlung: Kroko Classic mit Greedy decoding, 20-ms input chunks und einem
CPU-Inferenzthread betreiben.** Grundlage ist der abgeschlossene Benchmark vom
3. November 2026 auf einem Intel N100: **2.400 Messungen, keine Fehler und keine
Timeouts**. Es wurden keine Parameter nachträglich auf ein gewünschtes Ergebnis
optimiert. Die Untersuchung änderte weder die produktive Runtime noch Modelle.

## Versuchsaufbau

| Eigenschaft | Dokumentierte Einstellung |
| --- | --- |
| CPU / System | Intel N100, Linux x86-64 |
| Ausführung | CPU only; ein Inferenzthread; CPU-Affinität auf CPU 3 |
| Modell | Kroko Classic: sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06 |
| Export-Revision | `887db3d083240198c2d2b99fb66cfcfe6948ced8` |
| Runtime | sherpa-onnx 1.13.8, Git `8c8e275dd7f59cefe787af04c47e43fada0129ae`; ONNX Runtime 1.28.2 |
| Benchmark-Adapter | Isolierter direkter sherpa-C-API-Aufruf mit Python-Instrumentierung |
| Audio | 16 kHz, mono, PCM16; für native Aufrufe in Float-Samples konvertiert |
| Features | 80 Dimensionen; Feature-Schritt 10 ms |
| Datensatz | Dieselben 100 vollständigen deutschen FLEURS-Aufnahmen wie im vorherigen Modellvergleich |
| Wiederholungen | Drei je Variante und Betriebsart |
| Umfang | 4 Varianten × 100 Aufnahmen × 3 Wiederholungen × 2 Betriebsarten = 2.400 |
| Zusätzliche Tests | 24 ausgeschlossene Warmups und vier Funktionstests |
| Parallelität | Varianten seriell; keine parallelen ASR-Messungen |
| Modellkontext | Unverändert; interner Encoder-Schritt 128 Featureframes = 1,28 s; Eingabefenster 141 Frames |
| Gemeinsame Einstellungen | Endpoint-Erkennung aus, Blank-Penalty 0, kein externes Sprachmodell, keine Hotwords |
| Abschluss | 23.040 Nullsamples = 1,44 s Silence-Tail, danach InputFinished und vollständiges Decode-Drain |

In jedem Messblock wurde ein eigener Prozess gestartet, das Modell einmal geladen
und eine Warmup-Anfrage ausgeschlossen. Für jede Aufnahme wurde ein neuer Stream
angelegt. Die Reihenfolge der Aufnahmen war deterministisch randomisiert;
Greedy 20 ms lief in jeder Wiederholung zuerst. Die Reihenfolge der übrigen drei
Varianten war vorab deterministisch festgelegt. Die Hintergrundlast des
Unraid-Hosts wurde nicht abgeschaltet; Messwerte können von Takt, Cache- und
Allocatorzustand sowie anderen Host-Aktivitäten abhängen.

Die Modellkomponenten und ihre SHA256-Prüfungen sind im vorhandenen Modellprofil
in [src/server/models.rs](../src/server/models.rs) festgelegt; die Bereitstellung
beschreibt [docs/models.md](models.md). Der Server lädt keine Modelle automatisch.
Modelllizenz und zulässige Nutzung bleiben ungeklärt; dieser Bericht erteilt keine
Nutzungs- oder Weiterverteilungsrechte. Siehe [docs/licensing.md](licensing.md).

## Vier unveränderte Varianten

| Variante | Decoder | Audio input chunk | max_active_paths |
| --- | --- | --- | --- |
| Greedy 20 ms | `greedy_search` | 320 Samples / 20 ms | 4; bei Greedy ohne Wirkung |
| Greedy 320 ms | `greedy_search` | 5.120 Samples / 320 ms | 4; bei Greedy ohne Wirkung |
| Beam 4 | `modified_beam_search` | 320 Samples / 20 ms | 4 |
| Beam 8 | `modified_beam_search` | 320 Samples / 20 ms | 8 |

Alle übrigen Einstellungen, Gewichte und Audioeingaben blieben identisch.
Die angegebenen Blockgrößen betreffen die Anwendung bzw. den sendenden Client.
**Sie verändern weder den festen internen Encoder-Schritt von 1,28 s noch den
Modellkontext.** Die produktive Rust-Runtime verwendet bereits Greedy und bietet
keinen durch diese Dokumentation neu eingeführten Beam-Schalter. `NUM_THREADS=1`
ist die vorhandene Thread-Einstellung; die Audio-Paketierung bestimmt der Client.

## Ergebnisse im Überblick

RTF stammt ausschließlich aus Datei-Inferenz. Streaming-Latenzen stammen aus der
separaten Echtzeit-Simulation. Peak RAM ist konservativ das Maximum der
beobachteten Prozess-RSS-Peaks beider Betriebsarten.

| Variante | WER | CER | Median RTF | P95 RTF | Peak RAM | First stable 3-word partial, Median / P95 | Final latency, Median / P95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Greedy 20 ms | **7,85 %** | **4,57 %** | **0,0719** | 0,1243 | 178,7 MiB | 4,073 / 7,913 s | **75,7 / 148,2 ms** |
| Greedy 320 ms | 7,85 % | 4,57 % | 0,0733 | **0,1177** | 181,3 MiB | 4,235 / 8,075 s | 77,4 / 154,6 ms |
| Beam 4 | 7,98 % | 4,64 % | 0,0778 | 0,1312 | 182,1 MiB | 4,078 / 7,919 s | 81,8 / 160,8 ms |
| Beam 8 | 7,98 % | 4,62 % | 0,0805 | 0,1351 | 185,4 MiB | 4,085 / 7,933 s | 89,0 / 179,4 ms |

Jede Zeile umfasst 100 unterschiedliche Aufnahmen und 300 gewertete Anfragen
je Betriebsart. WER/CER waren über Wiederholungen deterministisch; Datei- und
Streaming-Modus lieferten je Variante identische normalisierte Transkripte.
Alle 600 Greedy-20-ms-Transkripte bestanden zusätzlich den Vergleich gegen die
historische normalisierte Classic-Baseline.

## Bedeutung der Metriken

### Erkennungsqualität und Datei-Inferenz

WER und CER sind mikroaggregierte Fehlerquotienten: Summe der Editierfehler geteilt
durch die Summe der Referenzwörter bzw. -zeichen. Normalisierung: Unicode NFC,
Kleinschreibung, Interpunktion/Symbole zu Leerzeichen, Leerraum vereinheitlicht.
Umlaute, ß und Ziffern bleiben erhalten; CER zählt Leerzeichen mit.

Datei-RTF ist die gemessene Anfragezeit bis zum finalen Text geteilt durch die
ursprüngliche Audiodauer. Modellladen sowie WAV-Lesen und PCM-Konvertierung sind
nicht enthalten. Die Audioeingabe wird in diesem Modus ohne Echtzeit-Wartepausen
verarbeitet. CPU-Zeit umfasst die Erkennungsanfrage einschließlich Adapter und
Sampler. 100 % CPU entsprechen einem Kern.

| Variante | Wortfehler / Referenzwörter | Zeichenfehler / Referenzzeichen | Mittlere RTF | P95 Anfragezeit | CPU-Zeit gesamt | Mittlere CPU | Peak RSS Datei |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Greedy 20 ms | 534 / 6.804 | 2.094 / 45.867 | 0,0836 | 2,266 s | 341,40 s | 98,5 % | 178,7 MiB |
| Greedy 320 ms | 534 / 6.804 | 2.094 / 45.867 | 0,0841 | 2,212 s | 343,92 s | 98,9 % | 179,6 MiB |
| Beam 4 | 543 / 6.804 | 2.127 / 45.867 | 0,0898 | 2,447 s | 369,08 s | 98,7 % | 176,0 MiB |
| Beam 8 | 543 / 6.804 | 2.121 / 45.867 | 0,0931 | 2,670 s | 386,87 s | 99,0 % | 176,8 MiB |

Die Fehlerzahlen enthalten drei Wiederholungen derselben 100 Aufnahmen. Sie
stellen keine 300 unabhängigen Sprachbeispiele dar.

### Simuliertes Echtzeit-Streaming

Audio wurde mit der ursprünglichen Abtastrate zeitlich gepacet übergeben. Nach
dem letzten echten Audioblock wurde derselbe Silence-Tail ohne Wartepausen
zugeführt. Die Echtzeit-Wall-RTF enthält deshalb absichtliches Warten auf Audio
und ist keine reine Inferenzgeschwindigkeit.

**First stable 3-word partial** ist der Zeitpunkt, an dem mindestens drei
vollständige Anfangswörter dem finalen Text entsprechen und durch kein späteres
Update geändert oder entfernt werden. Das letzte Token einer nichtfinalen
Hypothese wurde vorsichtshalber als möglicher Wortrest ausgeschlossen. Diese
rückblickende Stabilitätsmetrik ist kein menschliches Urteil über den Inhalt.

**Die etwa vier Sekunden bedeuten ausdrücklich nicht, dass Kroko erst dann die
ersten Wörter erkennt.** Das erste nichtleere Teilergebnis ist eine eigene,
weniger strenge Metrik und kann noch Fragmente oder später veränderte Wörter
enthalten. Alle 300 Anfragen jeder Variante erreichten das stabile Drei-Wort-
Partial vor dem Ende der Originalaufnahme.

**Final latency** beginnt bei der tatsächlichen Übergabe des letzten echten
Audioblocks und endet beim finalen nativen Transkript. Sie enthält die Verarbeitung
dieses Blocks und den Abschluss mit Silence-Tail. Sie misst weder Netzwerk- und
Wyoming-Auslieferung noch Home-Assistant- oder VAD-/Endpoint-Latenz. Der Client
signalisiert hier ein bekanntes Dateiende; ein offenes Mikrofon wurde nicht getestet.

| Variante | Erstes nichtleeres Partial, Median / P95 | EOF-Flush, Median / P95 | CPU-Zeit gesamt | Mittlere CPU | Peak RSS Streaming |
| --- | --- | --- | --- | --- | --- |
| Greedy 20 ms | 2,794 / 6,632 s | 75,5 / 147,4 ms | 284,06 s | 6,8 % | 178,6 MiB |
| Greedy 320 ms | 2,954 / 6,791 s | 75,4 / 147,3 ms | 271,50 s | 6,5 % | 181,3 MiB |
| Beam 4 | 2,798 / 6,636 s | 81,7 / 160,1 ms | 301,95 s | 7,2 % | 182,1 MiB |
| Beam 8 | 2,807 / 6,642 s | 88,7 / 178,5 ms | 323,78 s | 7,7 % | 185,4 MiB |

EOF-Flush beginnt erst nach Verarbeitung des letzten echten Audioblocks und ist
daher von Final latency zu unterscheiden. Native Partials wurden nach internen
Decode-Schritten beobachtet; sie sind keine an Home Assistant ausgelieferten
Partial-Ereignisse.

### RAM und Vergleichsgrenzen

RSS wurde alle 100 ms abgetastet und enthält den Python/C-API-Adapter. Kurze Peaks
können der Abtastung entgehen; zusätzlich wurde der Prozess-VmHWM kontrolliert.
Der Wert ist weder Docker-Speicherverbrauch noch Start-RSS der Rust-Runtime.
Unterschiede von wenigen MiB können auch vom Allocator-/Prozesszustand abhängen.

Frühere Classic-Werte von 0,0741 Median-RTF und etwa 161 MiB stammen aus einem
separaten Rust/Wyoming-Benchmark. Dort war RAM ein Mittelwert von Anfrage-Peaks;
hier wird das Maximum gemeldet. Unterschiedliche Adapter und Messgrenzen erlauben
keine direkte Gleichsetzung dieser Geschwindigkeits- und Speicherwerte.

## Vergleiche und Partial-Stabilität

### A: Greedy 20 ms gegenüber Greedy 320 ms

In beiden Betriebsarten waren alle 300 gepaarten Rohtranskripte, normalisierten
Transkripte und Decode-Zahlen identisch. WER und CER bleiben unverändert.
320-ms-Blöcke erzeugen keinen zusätzlichen Modellkontext; der Median des stabilen
Drei-Wort-Partials liegt um 161,3 ms später, sein P95 um 161,8 ms.

Die Median-Datei-RTF steigt um 1,92 %. Der P95-Datei-RTF und die CPU-Zeit im
gepaceten Streaming fallen bei 320 ms etwas günstiger aus. Diese Unterschiede
begründen hier keinen praktischen Vorteil für interaktive Erkennung: gleicher
Text, aber später sichtbare stabile Partials. 20 ms bleibt die Empfehlung.

### B/C: Greedy gegenüber Modified Beam Search

| Änderung gegenüber Greedy 20 ms | Beam 4 | Beam 8 |
| --- | --- | --- |
| WER-Veränderung | +0,132 Prozentpunkte | +0,132 Prozentpunkte |
| Relative WER-Veränderung | +1,69 % Fehlerquote | +1,69 % Fehlerquote |
| CER-Veränderung | +0,072 Prozentpunkte | +0,059 Prozentpunkte |
| Median-Datei-RTF-Aufschlag | +8,23 % | +11,99 % |
| Median stabiles Partial, Aufschlag | +5,0 ms | +12,0 ms |
| P95 stabiles Partial, Aufschlag | +6,4 ms | +20,6 ms |
| Median Final latency, Aufschlag | +6,2 ms | +13,3 ms |
| P95 Final latency, Aufschlag | +12,6 ms | +31,1 ms |
| Peak RSS Streaming, Aufschlag | +3,5 MiB | +6,8 MiB |

Je 100 Aufnahmen erzeugt Greedy 178 Wortfehler, Beam jeweils 181. Beide
Beam-Varianten verbessern bei einer Aufnahme die Wortfehlerzahl, verschlechtern
vier und belassen sie bei 95 unverändert. Es gibt keinen gemessenen Qualitätsgewinn.

Gepaarter Bootstrap: 20.000 Resamples der 100 Aufnahmen, Seed 20261103; die Fehler
pro Aufnahme wurden über Wiederholungen gemittelt. Das explorative 95-%-Intervall
der WER-Verbesserung gegenüber Greedy beträgt bei Beam 4 −0,334 bis +0,045 und bei
Beam 8 −0,333 bis +0,045 Prozentpunkte. Beide Intervalle umfassen null: Auch eine
allgemeine Verschlechterung ist nicht statistisch bewiesen. Ein relevanter
Genauigkeitsvorteil wird durch diesen Test nicht gestützt. Die vier Paarvergleiche
wurden nicht für Mehrfachtests korrigiert.

### D: Beam 4 gegenüber Beam 8

Beam 8 erreicht dieselbe WER und pro 100 Aufnahmen nur zwei Zeichenfehler weniger.
Dafür steigen Median-Datei-RTF um 3,48 % und mediane Finalisierung um 7,2 ms.
Ein größerer Suchraum allein begründet keine Empfehlung für Beam 8.

### Revisionen vollständiger Wörter

| Variante | Streaming-Anfragen mit vollständigen Wortrevisionen | Unterschiedliche betroffene Aufnahmen | Partial-Updates, Median / P95 |
| --- | --- | --- | --- |
| Greedy 20 ms | 0 / 300 | 0 / 100 | 9,0 / 15,0 |
| Greedy 320 ms | 0 / 300 | 0 / 100 | 9,0 / 15,0 |
| Beam 4 | 3 / 300 | 1 / 100 | 9,0 / 15,0 |
| Beam 8 | 9 / 300 | 3 / 100 | 9,0 / 14,1 |

Gezählt werden Anfragen mit mindestens einer Substitution oder Löschung zwischen
aufeinanderfolgenden normalisierten Hypothesen; reines Anhängen zählt nicht als
Revision. Das letzte nichtfinale Token wird als möglicher Wortrest ausgeschlossen,
der finale Text als Abschluss einbezogen. Geänderte nichtleere Hypothesen und
Tail-Updates sind enthalten. Diese Editierdistanz-Näherung ist keine lückenlose
Identitätsverfolgung einzelner Wörter. Die bloße Vervollständigung eines Fragments
ist nicht gleichbedeutend mit einer Korrektur eines vollständigen Wortes.

## Historischer Kontext: Community-128

Die Community-128-WER von **7,28 %** stammt aus einem **separaten Modellbenchmark**.
Keine der hier getesteten Classic-Beam-Varianten erreicht diesen historischen
Punktwert. Die frühere Qualitätsmessung verwendete dieselben 100 Aufnahmen und
Normalisierung, aber einen anderen Adapter. Geschwindigkeit und RAM werden wegen
unterschiedlicher Messgrenzen ausdrücklich nicht direkt gleichgesetzt. Dieser
Decoding-Test ist kein neuer kontrollierter Classic-/Community-Lauf und untersucht
keine Beam-Konfiguration des Community-Modells.

## Reproduktion und Grenzen

Für eine entsprechende unabhängige Wiederholung:

1. Das identische Classic-Exportprofil mit den im Repository dokumentierten
   Prüfsummen verwenden; keine Gewichte oder ONNX-Graphen verändern.
2. Dieselben vollständigen 100 Aufnahmen, Referenzen und die oben beschriebene
   Audio- und Textnormalisierung verwenden. Das dokumentierte Auswahlmanifest hat
   SHA256 `a736253ca3790898c81759fb8a675919944f4dfda57b901803c0943b794fdca2`.
3. Die vier festgelegten Konfigurationen mit einem CPU-Inferenzthread, gleicher
   CPU-Affinität und drei Wiederholungen seriell ausführen. Pro Block laden und
   aufwärmen, pro Aufnahme einen neuen Stream erzeugen; Warmups ausschließen.
4. Datei-Inferenz ohne Pacing und Streaming mit dem Original-Sampletakt separat
   messen. Den identischen Silence-Tail unmittelbar zuführen und den Stream
   abschließen. Erste nichtleere und erste stabile Drei-Wort-Partials getrennt
   auswerten; Final latency auf die tatsächliche letzte Audioübergabe beziehen.
5. Fehlerquotienten aggregieren und die 100 Aufnahmen bei Qualitätsintervallen
   als unabhängige Einheiten behandeln, nicht ihre Wiederholungen.

Das Auswahlmanifest, die Audio-/Referenzdateien, die Rohmessungen und der separate
Benchmark-Harness sind nicht Bestandteil dieses öffentlichen Repositories. Der
Quellbericht legt keine öffentliche FLEURS-Dataset-Revision bzw. vollständig
öffentliche Auswahlprozedur fest. Der Manifest-Hash identifiziert die verwendete
Auswahl, ersetzt aber nicht deren Inhalt. Ohne die Originalauswahl lässt sich der
exakte Lauf daher nicht allein aus diesem Repository reproduzieren. Beliebige
andere 100 FLEURS-Aufnahmen sind keine identische Wiederholung. Die Anleitung
beschreibt die dokumentierten Bedingungen und behauptet keinen mitgelieferten
Benchmark-Befehl oder Beam-Schalter für den Produktionsserver.

## Empfehlung für den Intel N100

- **Genauigkeit:** Greedy 20 und 320 ms teilen die besten WER-/CER-Werte.
- **Geschwindigkeit:** Greedy 20 ms hat die niedrigste Median-Datei-RTF; Greedy
  320 ms den niedrigsten P95-Datei-RTF.
- **Qualität/Geschwindigkeit:** Greedy 20 ms benötigt keinen Beam-Aufschlag und
  zeigt keine Revision vollständiger Wörter.
- **Interaktive Home-Assistant-Nutzung:** Greedy 20 ms vermeidet die zusätzliche
  Paketwartezeit von 320 ms; ein CPU-Thread genügte im untersuchten N100-Test.

**Kroko Classic: Greedy decoding + 20-ms input chunks + 1 CPU thread.**
Die Empfehlung basiert auf dem abgeschlossenen 2.400-Messungen-Benchmark. Sie ist
keine Leistungsgarantie für andere Systeme oder alle Sprachbefehle; FLEURS ist
kein spezifischer Home-Assistant-Befehlskorpus. Es wurden keine Home-Assistant-,
Netzwerk- oder VAD-Latenzen gemessen. Modell- und Drittanbieterlizenzen bleiben
unabhängig von der Apache-2.0-Lizenz dieses Projekts zu prüfen.
