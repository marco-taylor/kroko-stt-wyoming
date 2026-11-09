<img src="icons/kroko-stt-wyoming.png" alt="Kroko STT Wyoming Icon" width="160" height="160">

# Kroko STT Wyoming

Mehrsprachige lokale Streaming-Spracherkennung für Home Assistant mit Rust,
sherpa-onnx und Wyoming. CPU-only, linux/amd64, standardmäßig ein Thread;
keine Python-Laufzeit, keine GPU und keine zusätzliche Inferenz-Runtime.

## Installation

1. In Unraid Community Apps **Kroko STT Wyoming** installieren.
2. Sprache wählen (`de` ist Standard), **Community-128** belassen und den
   persistenten Modellordner nach `/models` einbinden.
3. Container starten: Nur das ausgewählte fehlende Community-Modell wird direkt
   vom Originalanbieter geladen, per gepinntem SHA256 geprüft und vorbereitet.
4. In Home Assistant **Wyoming Protocol** hinzufügen: Unraid-Hostadresse und den
   gewählten freien Host-Port eintragen; anschließend `kroko` in Assist wählen.

Neuinstallation: `ghcr.io/marco-taylor/kroko-stt-wyoming:0.3.1` oder `:latest`.
Modelle liegen persistent außerhalb des Images. Neustart und Containerupdate
laden vollständige gültige Modelle nicht erneut herunter. Nach erfolgreicher
Installation funktionieren Start und Spracherkennung auch offline und read-only.

Der deutsche Erstdownload benötigt ungefähr 156 MB. Nach vollständig verifizierter
Vorbereitung wird das redundante freie Community-Paket entfernt; ONNX-Komponenten
und Prüfmetadaten bleiben erhalten. Während der Installation ist vorübergehend
zusätzlicher Speicher für Paket und Komponenten nötig. Keine automatische
Reparatur oder Ersetzung ungültiger Bestandsdateien.

## Sprache und Modell

`LANGUAGE` und `MODEL` bilden gemeinsam eine eindeutige Identität. `KROKO_MODEL`
ist ein rückwärtskompatibler Alias für `MODEL`; beide dürfen sich nicht widersprechen.
Eine Serverinstanz lädt genau ein Modell. Ein Wechsel erfordert angepasste
Umgebungsvariablen und Neuerstellen des eigenen Containers.

| Auswahl | Persistenter neuer Modellpfad | Bezug |
| --- | --- | --- |
| `LANGUAGE=de`, `MODEL=community-128` | `/models/de/community-128/` | Standard, bei Bedarf automatisch |
| `LANGUAGE=en`, `MODEL=community-128` | `/models/en/community-128/` | Bei Bedarf automatisch |
| `MODEL=community-64` | `/models/<sprache>/community-64/` | Optional, nur wenn ausgewählt |
| `LANGUAGE=de`, `MODEL=classic` | `/models/de/classic/` oder bestehender Legacy-Pfad | Nur manuell/lokal |
| `MODEL=custom/<name>` | `/models/<sprache>/custom/<name>/` | Lokal mit Manifest |

Community-Sprachen: Deutsch, Englisch, Spanisch, Französisch, Italienisch,
Hebräisch, Niederländisch, Portugiesisch, Schwedisch und Türkisch.
Schwedisch wird nur mit Community-64 angeboten. Classic bleibt Deutsch.
Nicht unterstützte Kombinationen brechen ohne Sprachfallback ab.
Die [Modellmatrix](docs/models.md) unterscheidet verfügbare und nativ getestete
Kombinationen; der [offizielle Katalog](src/server/model_catalog.json) pinnt Quellen,
Dateigrößen und Prüfsummen. Keine automatische Spracherkennung.

Community-128 erreichte im separaten deutschen Vergleich 7,28 % WER,
Classic 7,85 %. Classic bleibt die schnellere optionale Alternative; diese
100 Aufnahmen sind keine allgemeine Qualitätsgarantie.

## Manuell, offline und Custom

Vollständige lokale Modelle werden vor jedem Download geprüft und wiederverwendet.
`MODEL_AUTO_DOWNLOAD=false` erzwingt lokalen Betrieb. Classic benötigt die vier
bestehenden Dateien Encoder, Decoder, Joiner und Tokens; alte deutsche Modellpfade
funktionieren weiter. Kein Classic-Download und keine erzwungene Migration.

Vorbereitete Custom-Modelle verwenden ein lokales Manifest mit Sprache,
Backend und Dateizuordnung. Freie lokale Kroko-Pakete lassen sich ausdrücklich
mit `MODEL=custom/<name>` und `CUSTOM_MODEL_PATH=/models/<datei>.data` auswählen.
Die einmalige Vorbereitung übernimmt der Container. Manuelle Custom-Originale
bleiben erhalten. Es gibt keine mehrdeutige automatische Dateiauswahl und keinen
öffentlichen Downloader für externe Custom-Modelle.

[Installation, Manifest, Prüfungen und sichere Entfernung](docs/models.md).
Modelle sind weder im Repository noch im Image. Die Community-Lizenz lautet
CC-BY-SA, Version vom Anbieter nicht angegeben; Classic-Modellrechte bleiben
separat zu klären. [Lizenzhinweise](docs/licensing.md).

## Upgrade von Classic

Bestehende Classic-Installationen funktionieren weiter. **Keine automatische
Migration, kein Modellersatz und kein notwendiger Community-Download.**
`KROKO_MODEL=classic` und den bestehenden persistenten Modell-Mount beim
Containerupdate beibehalten. Ohne explizite Auswahl erkennt der Manager auch
vorhandenes Classic einschließlich der bisherigen Verzeichnisformen.
Community-128 ist ausschließlich der Default für neue/leere Installationen
bzw. neue Standardkonfigurationen.

Wer einen bestehenden Container manuell mit einer neuen CA-Vorlage neu anlegt,
muss die bisherige Auswahl `classic` übernehmen: die Neuinstallationsvorlage
setzt ausdrücklich `community-128`. Ein normaler Image-Update mit gespeicherter
Konfiguration ändert die Auswahl nicht. Vollständige vorhandene Classic-Modelle
können weiterhin read-only eingebunden bleiben.

## Model-Manager-Befehle

Im Unraid-Terminal, mit dem Namen des eigenen laufenden Containers
anstelle von `YOUR_KROKO_CONTAINER` (das Distroless-Image enthält keine Shell):

```sh
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager list
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager status
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager verify community-128
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager install community-64
```

`list`, `status` und `verify` sind offline und ändern keine Dateien. `install`
bezieht ausschließlich fehlende Community-Modelle und ersetzt keine
Bestandsdateien. `remove <ID> --yes` entfernt gezielt ein Modell; ein gerade
verwendetes Modell ist gesperrt. Den sicheren Aufruf für einen gestoppten Server
und die Folgen eines späteren Neustarts beschreibt
[Ein Modell gezielt entfernen](docs/models.md#cli-und-sichere-entfernung).

Release Notes: [v0.3.1](docs/releases/v0.3.1.md); [v0.3.0](docs/releases/v0.3.0.md); [v0.2.0](docs/releases/v0.2.0.md) bleibt unverändert.

## Lokaler Docker-Build und Start

Das Dockerfile verwendet festgelegte Versionen bzw. Prüfsummen für Rust 1.90.0,
sherpa-onnx 1.13.8, dessen Quellarchiv und die Basis-Images. Cargo-Abhängigkeiten
sind gesperrt. Für den Build ist Internetzugriff erforderlich, für die eigentliche
Spracherkennung zur Laufzeit nicht. Das finale Distroless-Image läuft als UID
99 / GID 100 (Unraid nobody:users), enthält keine Modelle, Audiodateien, Python-Laufzeit oder Build-Werkzeuge
und nutzt ausschließlich die CPU.

Unter Unraid entspricht UID 99 / GID 100 `nobody:users`. Für den automatischen
Erststart benötigt ausschließlich der persistente `/models`-Mount Schreibrechte.
Bereits vollständige Classic-/Community-/Custom-Dateien können weiterhin `ro`
bleiben. Der Manager verändert vorhandene Rechte nicht automatisch. Ein neues
Image oder geänderte Containerparameter erfordern das Neuerstellen des Containers;
ein bloßer Neustart übernimmt sie nicht.

Dieser Abschnitt beschreibt den optionalen lokalen Build. Für die
Weiterverteilung eines fertigen Binaries bzw. Images ist die Lizenzprüfung aus
[docs/licensing.md](docs/licensing.md) zu beachten.

Der native Build deaktiviert TTS und Sprecherdiarisierung und verwendet das
offizielle Rust-Feature `shared`. eSpeak NG, Piper-Phonemisierung und ucd-tools
werden nicht eingebunden. Das Image bewahrt die Lizenztexte der Abhängigkeiten
und enthält die exakten glibc-/Eigen-Quellen als Lizenz-Compliance-Material unter
`/usr/share/licenses/kroko/dependencies/corresponding-source`.
Diese Quellarchive sind keine ASR-Modelle oder kompilierten Build-Artefakte.

```sh
./scripts/build-image.sh
MODELS_DIR=/absolute/path/to/your/models ./scripts/run-local.sh
```

Das Hilfsskript verwendet standardmäßig den Loopback-Port 10321 und ersetzt
keinen bereits vorhandenen Container. Soll Home Assistant von einem anderen
Rechner zugreifen, müssen ein freier Port und eine erreichbare Bind-Adresse
angegeben werden, zum Beispiel `BIND_ADDRESS=0.0.0.0 HOST_PORT=10321`.

Wyoming TCP bietet selbst keine Authentifizierung und kein TLS. Der Dienst sollte
daher nur im vertrauenswürdigen lokalen Netzwerk erreichbar sein. Nur das persistente Modellvolume benötigt für Installation Schreibrechte;
Docker-Socket und privilegierte Ausführung werden nicht benötigt.

In Home Assistant die Integration **Wyoming Protocol** mit der erreichbaren
Adresse des Docker-Hosts und dem gewählten Port hinzufügen. Anschließend `kroko`
als STT-Anbieter in der Assist-Pipeline auswählen. Dieses Projekt verändert die
Home-Assistant-Konfiguration nicht automatisch.

| Umgebungsvariable | Container-Standard | Bedeutung |
| --- | --- | --- |
| `KROKO_MODEL` | Community-128 bei Neuinstallation | Auswahl; vorhandenes Classic ohne explizite Auswahl bleibt erhalten |
| `MODEL_AUTO_DOWNLOAD` | `true` | Fehlende Community-Modelle automatisch beziehen; `false` für strikt lokalen Start |
| `MODEL_DIR` | `/models` | Persistente Wurzel für bekannte Profile und `custom/<name>` |
| `HOST` | `0.0.0.0` | Listener-Adresse im Container |
| `PORT` | `10321` | Wyoming-TCP-Port |
| `NUM_THREADS` | `1` | Inferenz-Threads, zulässig sind 1–4 |
| `LANGUAGE` | `de` | Modell-Sprache, z.B. `de` oder `en`; regionale Aliase werden normalisiert. |
| `LOG_LEVEL` | `info` | `error`, `warn`, `info`, `debug` |
| `TZ` | `Europe/Berlin` | Zeitzone des Containers |

Die Standardwerte bei direkter Ausführung unterscheiden sich:
`MODEL_DIR=models`, `HOST=127.0.0.1`, `PORT=10300`.
Für die native Entwicklung wird das passende sherpa-Bibliotheksverzeichnis benötigt:

```sh
SHERPA_ONNX_LIB_DIR=/absolute/path/to/native/lib cargo test --locked
SHERPA_ONNX_LIB_DIR=/absolute/path/to/native/lib cargo build --release --locked --bins
```

Verwende die TTS-freien Shared-Bibliotheken aus dem Docker-Build. Beim nativen
Start muss ihr Verzeichnis außerdem über `LD_LIBRARY_PATH` erreichbar sein;
im Container ist dies bereits konfiguriert.

## Protokoll, Streaming und Datenschutz

Der Server beantwortet `describe` mit `info`, akzeptiert `transcribe`,
`audio-start`, mehrere `audio-chunk`-Ereignisse und anschließend
`audio-stop`. Danach wird `transcript` zurückgegeben.

Audio muss als 16 kHz, Mono, vorzeichenbehaftetes 16-Bit-Little-Endian-PCM
vorliegen. Ein Resampler ist nicht enthalten. TCP-Fragmentierung wird unabhängig
von den Ereignisgrenzen verarbeitet. Audio wird fortlaufend verarbeitet und
anschließend verworfen; der Dienst zeichnet keine Audiodaten auf.

Es gibt keine interne VAD. Der Client muss die Anfrage mit `audio-stop`
abschließen. Es läuft höchstens eine aktive Inferenz; eine gleichzeitige zweite
STT-Anfrage erhält einen `busy`-Fehler. Die Zahl der Verbindungen ist begrenzt.
Beim sauberen Beenden kann der Server bis zum Verbindungs-Timeout
warten, bevor Worker beendet werden.

Teilergebnisse dienen internen Diagnosezwecken und sind keine proprietären
Home-Assistant-Ereignisse. Bei `info` werden finale Transkripte protokolliert;
`debug` protokolliert zusätzlich geänderte Teilergebnisse. Mit
`LOG_LEVEL=warn` lassen sich normale Transkript-Logs vermeiden. Bei privaten
Sprachdaten sollte zusätzlich die Docker-Log-Aufbewahrung berücksichtigt werden.
Die Logs enthalten keine Audioaufzeichnungen.

Der mitgelieferte `wyoming-probe` dient als Healthcheck und optional als lokaler
WAV-Smoke-Test:

```sh
docker exec kroko-stt-wyoming-local /usr/local/bin/wyoming-probe
# Mit einer separat schreibgeschützt eingebundenen 16-kHz-Mono-PCM16-WAV:
# wyoming-probe --smoke 127.0.0.1:10321 /test/01.wav
```

Der Smoke-Test prüft die bekannten Regressionssätze in `01.wav` (Licht im
Wohnzimmer) und `03.wav` (Temperatur auf zweiundzwanzig Grad). Er ist kein
allgemeines WAV-Transkriptionswerkzeug. Testaufnahmen werden nicht mitgeliefert.

## Empfohlene Streaming- und Decoding-Einstellungen

Für **Kroko Classic** empfehlen wir auf dem **Intel N100, CPU only**:
**Greedy decoding + 20-ms input chunks + 1 CPU thread** (`NUM_THREADS=1`).
Classic und Community-64 verwenden weiterhin Greedy. Für das offizielle deutsche
**Community-128** ist **Modified Beam Search mit 4 aktiven Pfaden** der
qualitätsoptimierte Standarddecoder. Die Auswahl erfolgt automatisch nach
Sprache und Modell; andere Sprachen und Custom-Modelle behalten Greedy.
Es gibt keinen zusätzlichen Decoder-ENV-Schalter. Die Audioblockgröße bestimmt
der sendende Client; Streaming, Modellkontext und Finalisierung bleiben unverändert.

**20 ms ist die Größe der übergebenen Audioblöcke, nicht die interne
Modell-Schrittweite.** Classic besitzt eine feste interne Encoder-Schrittweite
von **1,28 s**. Größere 320-ms-Eingabeblöcke erweitern den Modellkontext nicht.
Modified Beam Search brachte bei **Classic** im abgeschlossenen Vergleich keinen
Genauigkeitsgewinn; dieser Befund gilt nicht für Community-128.

## Kroko-Classic-Decoding-Benchmark

Am 3. November 2026 wurden dieselben 100 deutschen FLEURS-Aufnahmen pro Variante
und Betriebsart dreimal getestet: **2.400 Messungen, keine Fehler, keine Timeouts**.
Datei-Inferenz und simuliertes Echtzeit-Streaming wurden getrennt und seriell in
einer isolierten Benchmark-Umgebung gemessen.

| Variante | WER | CER | Median RTF | P95 RTF | Peak RAM | First stable 3-word partial, Median / P95 | Final latency, Median / P95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Greedy 20 ms | **7,85 %** | **4,57 %** | **0,0719** | 0,1243 | 178,7 MiB | 4,073 / 7,913 s | **75,7 / 148,2 ms** |
| Greedy 320 ms | 7,85 % | 4,57 % | 0,0733 | **0,1177** | 181,3 MiB | 4,235 / 8,075 s | 77,4 / 154,6 ms |
| Beam 4, 20 ms | 7,98 % | 4,64 % | 0,0778 | 0,1312 | 182,1 MiB | 4,078 / 7,919 s | 81,8 / 160,8 ms |
| Beam 8, 20 ms | 7,98 % | 4,62 % | 0,0805 | 0,1351 | 185,4 MiB | 4,085 / 7,933 s | 89,0 / 179,4 ms |

**„First stable 3-word partial“ bedeutet: drei vollständige Anfangswörter waren
erkannt und blieben danach unverändert. Die rund vier Sekunden sind ausdrücklich
nicht die Zeit bis zur ersten Erkennung oder zum ersten nichtleeren Teilergebnis.**
Die Metrik wird rückblickend anhand aller späteren Hypothesen bestimmt.

RTF stammt aus Datei-Inferenz; die Latenzen stammen aus separat zeitlich gepacetem
Streaming. Final latency beginnt bei der tatsächlichen Übergabe des letzten
Audioblocks und enthält keine Home-Assistant-, Netzwerk- oder VAD-Latenz.
Peak RAM ist der höchste Prozess-RSS aus beiden Betriebsarten, einschließlich des
Python/C-API-Benchmark-Adapters; er ist kein Speicherwert des Rust-Containers.

Beam 4 erhöht die Median-RTF um **8,23 %**, Beam 8 um **11,99 %** gegenüber
Greedy 20 ms, jeweils bei **7,98 % statt 7,85 % WER**. Anfragen mit Revisionen
vollständiger Wörter: Greedy **0/300**, Beam 4 **3/300**, Beam 8 **9/300**.
320-ms-Blöcke liefern identische WER/CER, aber rund **161 ms spätere stabile
Partials** und damit keinen praktischen Vorteil für die empfohlene interaktive
Nutzung. Ihr etwas besserer P95-Datei-RTF ist in der Tabelle sichtbar.

Methodik, Metrikdefinitionen, CPU-Werte, Vergleichsgrenzen und Reproduzierbarkeit:
[docs/benchmark-classic-decoding.md](docs/benchmark-classic-decoding.md).
Die Ergebnisse sind keine Leistungsgarantie und der FLEURS-Korpus ist kein
spezifischer Home-Assistant-Befehlskorpus.

## Lizenzierung

Der von diesem Projekt selbst entwickelte Quellcode steht unter Apache-2.0;
siehe [LICENSE](LICENSE) und [NOTICE](NOTICE).

Abhängigkeiten und Modelle besitzen eigene, davon unabhängige Lizenzbedingungen;
siehe [docs/licensing.md](docs/licensing.md). Insbesondere wird **nicht behauptet,
dass die Classic-Modellgewichte frei weiterverbreitet werden dürfen**. Auch bei
den Community-Modellen müssen die jeweils tatsächlich anwendbaren Bedingungen
beachtet werden. Modellgewichte übernehmen nicht automatisch die Apache-2.0-Lizenz
dieses Projekts.

## Danksagungen

Dieses Projekt baut auf der Arbeit mehrerer Open-Source-Projekte und Communities auf:

- **Kroko / Banafo** – für die Kroko-Streaming-Spracherkennungsmodelle.
- **sherpa-onnx / k2-fsa** – für die effiziente Streaming-ASR-Laufzeit und den
  OnlineRecognizer.
- **Wyoming** – für das leichtgewichtige Sprachassistenten-Protokoll zur
  Integration mit Home Assistant.
- **Home Assistant** – für die Open-Source-Heimautomatisierungsplattform und das
  Assist-Ökosystem.

Offizielle Projekte:
[Kroko / Banafo](https://github.com/Banafo) ·
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) ·
[Wyoming](https://github.com/OHF-Voice/wyoming) ·
[Home Assistant](https://www.home-assistant.io/)

Kroko/Banafo, sherpa-onnx, Wyoming und Home Assistant sind unabhängige Projekte.
Ihre Namen und Marken gehören den jeweiligen Rechteinhabern. Die Danksagungen
begründen keine Partnerschaft, Unterstützung oder Empfehlung durch diese Projekte.

### Name und Upgrade-Kompatibilität

Ab v0.3.1 heißt das öffentliche Repository und Image `kroko-stt-wyoming`.
Neuinstallationen verwenden `ghcr.io/marco-taylor/kroko-stt-wyoming:0.3.1`
und den Appdata-Default `/mnt/user/appdata/kroko-stt-wyoming/models`.
Die interne Ablage bleibt `/models/<sprache>/<modell>/`.

Bestehende Container und gespeicherte Unraid-Konfigurationen werden **nicht**
umbenannt. Bestehende Modell-Mounts beibehalten, auch wenn deren Hostpfad noch
`kroko-stt-german-wyoming` heißt. Keine Dateien verschieben und keinen neuen
leeren Mount über einen vorhandenen Modellbestand legen. Classic-Auswahl beibehalten.
GitHub leitet die frühere Repository-Adresse nach der Umbenennung weiter.
Die alte Icon-Adresse bleibt zur Link-Kompatibilität verfügbar. Für Community
Applications gibt es genau eine App-Vorlage: `templates/kroko-stt-wyoming.xml`.
Ein zweites vollständiges XML als Alias ist nicht erforderlich und wird nicht
mehr veröffentlicht. Historische Vorlagen bleiben in den bisherigen Git-Tags
erhalten; gespeicherte Container und Modell-Mounts werden nicht geändert.
Ältere versionierte Images und Releases werden nicht überschrieben.

[Änderungen und Migration v0.3.1](docs/releases/v0.3.1.md).

[Veröffentlichungs- und Installationsaudit](docs/publication-audit.md).
