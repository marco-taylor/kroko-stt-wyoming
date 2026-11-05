# Modelle: Sprache, Installation und Migration

Ab v0.3.0: sprachgetrennte Modellverwaltung. Bestehende deutsche v0.2.0-
Installationen werden ohne erzwungene Migration weiterverwendet.

## Auswahl und unterstützte Kombinationen

`LANGUAGE` und `KROKO_MODEL` bilden eine Identität. `MODEL` ist ein gleichwertiger
Alias; beide Variablen dürfen nicht widersprüchlich gesetzt sein.

```text
LANGUAGE=en
MODEL=community-128
MODEL_DIR=/models
```

Die Instanz verwendet genau dieses Modell. Kein automatisches Erkennen der
Audiosprache und kein Laden aller Varianten. Ein Modell-/Sprachwechsel erfordert
geänderte Containerumgebung und Neuerstellen des eigenen Containers. Persistente
Gewichte bleiben erhalten; ein bloßer Neustart ändert die Umgebung nicht.

Der eingebettete [Katalog](../src/server/model_catalog.json) enthält ausschließlich
geprüfte Dateinamen, Größen und Paket-SHA256 vom offiziellen Banafo-Repository,
Revision `d45212aeb212dd66083dd22710c9954f40ff8cc1`:

| LANGUAGE | Sprache | Community-64 | Community-128 |
| --- | --- | --- | --- |
| `de` | Deutsch | angeboten | angeboten |
| `en` | Englisch | angeboten | angeboten |
| `es` | Spanisch | angeboten | angeboten |
| `fr` | Französisch | angeboten | angeboten |
| `it` | Italienisch | angeboten | angeboten |
| `he` | Hebräisch, Anbieter-Dateicode `IW` | angeboten | angeboten |
| `nl` | Niederländisch | angeboten | angeboten |
| `pt` | Portugiesisch | angeboten | angeboten |
| `sv` | Schwedisch | angeboten | **nicht angeboten** |
| `tr` | Türkisch | angeboten | angeboten |

Dies ist eine Angebots-/Katalogmatrix, keine Genauigkeitsgarantie und keine
Behauptung, dass jedes Modell bereits nativ getestet wurde. Tatsächlich geprüfte
Kombinationen sind im lokalen Testbericht ausgewiesen. Nicht im Katalog vorhandene
Kombinationen, insbesondere `en + classic` oder `sv + community-128`, werden
abgewiesen. Niemals stillschweigender Rückfall auf Deutsch oder eine andere Variante.

Primäre zweistellige Sprachcodes werden kleingeschrieben. Regionale Aliase wie
`de-DE` oder `en-US` verwenden dieselbe primäre Modellidentität. `iw` wird nach
`he` normalisiert. Das erzeugt keine regional spezialisierten Modelle.
`LANGUAGE` ist unabhängig von der Sprache der Benutzeroberfläche.

## Persistente Ablage und erster Start

```text
/models/
  de/
    community-128/
      encoder.onnx
      decoder.onnx
      joiner.onnx
      tokens.txt
      model.json
      installation.json
    community-64/
    classic/
    custom/
      example/
  en/
    community-128/
    community-64/
    custom/
```

Manuell gelieferte Community-Pakete und zunächst leere dedizierte Modellordner
können Komponenten auch unter `.runtime/<paket-sha256>/` innerhalb dieses
sprachgebundenen Modellordners vorbereiten. Sprache bleibt Teil des Elternpfads.
Verschiedene Sprachen verwenden keine gemeinsame Cache-Ablage, selbst wenn
Dateinamen oder Komponentenbytes ähnlich sein sollten.

Für eine Neuinstallation mit dem v0.3.0: Sprache und Modell wählen,
einen persistenten Modellordner schreibbar für UID 99/GID 100 nach `/models`
einbinden und starten. `MODEL_AUTO_DOWNLOAD=true` lädt nur ein fehlendes,
ausgewähltes Community-Paket aus dem offiziellen gepinnten Katalog.
Keine erfundenen URLs, keine Drittanbieter-Mirrors, keine vorsorglichen Downloads
von Community-64 bei gewähltem Community-128.

Die Vorbereitung prüft Paket-SHA256, Größe, freien Header und Sprache,
Blockgrenzen, Komponenten, Tokenformat und ONNX-Hülle. Die Export-Metadaten des
Encoders liefern die tatsächliche Schrittweite; keine Ableitung aus `64` oder
`128` im Namen. Der automatische Pfad unterstützt die nachgewiesene Zipformer2-
Exportform Version 1 mit `decode_chunk_len=128/256` und `T=Schritt+13`.
Für diese Formen gilt der bereits validierte Silence-Tail von Schritt+16
Featureframes. Andere Exportformen benötigen ein explizites Custom-Manifest.
Die interne Modell-Schrittweite wird nicht durch Konfiguration verändert.

Neue Pakete und Komponenten entstehen zunächst in einem privaten Staging-Ordner.
Nur vollständige geprüfte Ergebnisse werden atomar ohne Ersetzung veröffentlicht.
Danach werden die Komponenten erneut verifiziert und **erst dann** das freie
Community-Paket entfernt. `model.json` hält gemessene Komponenten-Hashes und
Exportparameter fest; `installation.json` bindet Sprache, Modell und Paket-SHA.
Bei späteren Starts werden diese Dateien wiederverwendet: kein Download und
keine erneute Extraktion.

Bei fehlgeschlagener Vorbereitung nach einem abgeschlossenen Download bleibt das
Paket im privaten Staging-Bereich zur Untersuchung erhalten. Es gilt niemals als
installiertes Modell. Eine Fehlermeldung nennt den Ort. Unterbrochene/unvollständige
Downloads werden nicht als fertige Pakete veröffentlicht. Kein automatisches
Überschreiben, Löschen oder Reparieren bestehender Komponenten.

## v0.2.0 wiederverwenden

Bestehende deutsche Modelle werden ohne obligatorische Migration erkannt:

- `/models/community-128/` und `/models/community-64/`;
- `/models/classic/`;
- das bisherige lange Classic-Exportverzeichnis;
- flache Classic-Komponenten direkt in `MODEL_DIR`;
- `/models/custom/<name>/` mit deutschem Manifest.

Diese Legacy-Suche gilt **nur für Deutsch**. Bei Englisch wird niemals ein altes
deutsches Modell als vorhandenes englisches Modell behandelt. Eine vorhandene
neue sprachgetrennte Ablage hat Vorrang; ist sie ungültig, wird nicht heimlich auf
ein anderes Modell zurückgegriffen. Ohne explizite Auswahl bleibt vorhandenes
deutsches Classic vorrangig. Eine gewählte Sprache wird nicht automatisch gewechselt.

Gültige alte ONNX-Komponenten werden direkt verwendet; kein neuer Download und
kein Verschieben nötig. Ein zusätzliches korrekt identifiziertes freies Community-
Paket kann nach Verifizierung entfernt werden, wenn das Modellverzeichnis
schreibbar und nicht durch einen anderen Nutzer gesperrt ist. Bei read-only oder
belegtem Verzeichnis bleibt diese redundante Kopie erhalten; der Start bleibt
möglich. Ungültige zusätzliche Pakete werden nicht gelöscht.

Classic bleibt nur `de`, lokal/manuell, ohne automatischen Download. Die bekannten
Classic-SHA256 bleiben unverändert. Eine neue Version benennt vorhandene Container,
Mounts oder gespeicherte Unraid-Templates nicht selbst um.

## Offline und read-only

`MODEL_AUTO_DOWNLOAD=false` verhindert automatische Downloads beim Serverstart.
Ein vollständiges lokales Modell startet unabhängig davon ohne Internet.
Nach erfolgreicher Vorbereitung funktionieren Komponenten mit read-only Modellmount.
Die erste Vorbereitung benötigt Schreibrechte. Diagnosebefehle schreiben nichts.
Der Container läuft weiterhin als UID 99/GID 100, CPU only, normalerweise ein Thread.
Keine automatische globale Rechte-Reparatur und kein `chmod 777`.

## Custom Models

Vorbereitete Modelle: `/models/<sprache>/custom/<name>/model.json` und die darin
benannten Komponenten. Beispiel: [Custom-Manifest](examples/custom-model.json).
`language` muss zu `LANGUAGE` passen. Vier Rollen: Encoder, Decoder, Joiner und
Tokens; relative sichere Pfade und optional SHA256. German-spezifische Beispiele
sind Beispiele, kein globaler Sprachzwang. Symlinks und Traversal werden abgewiesen.

Freie lokale Kroko-Pakete ausdrücklich auswählen:

```text
LANGUAGE=de
MODEL=custom/local-package
CUSTOM_MODEL_PATH=/models/Kroko-DE-Community-128-L-Streaming-001.data
```

Es wird nur diese Datei verwendet, auch wenn weitere `.data`-Dateien vorhanden
sind. Bekannte katalogisierte Pakete übernehmen verifizierte Parameter; andere
freie Exporte benötigen ein sprachlich passendes Manifest. Keine manuelle
Komponentenextraktion durch den Benutzer. Der Cache liegt sprachgetrennt unter
`/models/<sprache>/custom/<name>/.runtime/<paket-sha256>/`.

**Manuelle Custom-Originale bleiben erhalten** und werden beim Neustart zur
Validierung benötigt. Der automatische Community-Cleanup löscht sie nicht.
`CUSTOM_MODEL_PATH` muss für normale Built-in-Profile und bereits vorbereitete
ONNX-Custom-Modelle leer sein. Ein Dateipfad muss innerhalb `MODEL_DIR` liegen;
ein Hostpfad oder Verzeichnis genügt nicht. Mehrere Dateien werden nie automatisch
erraten. Unbekannte Backends werden diagnostiziert, aber nicht automatisch gestartet.

## Nemotron 3.5 als vorbereitetes Custom-Modell

Ab v0.3.0 können die offiziellen multilingualen
sherpa-onnx-Nemotron-3.5-Streaming-INT8-Exporte über dasselbe Custom-Manifest
verwendet werden. Dies ist kein automatischer Nemotron-Installer. Die vier Dateien
bleiben unverändert unter ihren Originalnamen im ausgewählten Custom-Ordner:

```text
/models/de/custom/nemotron-560/model.json
/models/de/custom/nemotron-560/encoder.int8.onnx
/models/de/custom/nemotron-560/decoder.int8.onnx
/models/de/custom/nemotron-560/joiner.int8.onnx
/models/de/custom/nemotron-560/tokens.txt
```

`LANGUAGE=de-DE`, `MODEL=custom/nemotron-560` und `MODEL_AUTO_DOWNLOAD=false`
auswählen; `CUSTOM_MODEL_PATH` bleibt leer. Die Modellwurzel bleibt `/models`.
Für 1120 ms einen separaten Custom-Namen und den entsprechenden Export verwenden.
Quellen und angebotene Exporte: [offizielle sherpa-onnx-Anleitung](https://k2-fsa.github.io/sherpa/onnx/nemo/nemotron-streaming.html).

Zusätzlich zu den bisherigen Manifestfeldern werden benötigt:

```json
{
  "model_type": "nemo_transducer",
  "feature_dim": 128,
  "stream_language": "de-DE",
  "tail_samples": 4800
}
```

Dies ist ein Ausschnitt, kein vollständiges Manifest. Backend und Format bleiben
`sherpa-onnx-transducer` und `onnx-transducer`. Die vier Rollen verweisen auf die
Originaldateien; SHA256 und die tatsächliche Modelllizenz separat eintragen.
`nemo_transducer` bezeichnet hier unser Manifestprofil. Die native CPU-Runtime
erkennt die NeMo-Familie selbst; sie bekommt keine erzwungene Zipformer-Kennung.

Der Manager prüft tatsächliche Encoder-Metadaten, Featurezahl, Vocabulary-Größe
und Sprach-Prompt. Der NeMo-Blank liegt am Vokabularende; er wird nicht auf null
umgeschrieben. `stream_language` wird vor dem ersten Audioblock am Stream gesetzt.
Der Encoder dieses Exports überschreitet 512 MiB; nur das explizite NeMo-Profil
hat deshalb ein Dateilimit von 1 GiB. Kroko-Paketlimits bleiben unverändert.

Alte Manifeste behalten standardmäßig 80 Features und keine NeMo-Sprachoption.
Die interne Chunk-Konfiguration stammt aus dem jeweiligen Modell, nicht aus der
Größe der eingehenden Audioblöcke. Keine beliebigen Chunkwerte erzwingen.
Audio bleibt 16-kHz-Mono-PCM16, CPU-only, standardmäßig ein Thread und Greedy.
Die Wyoming-Schnittstelle bleibt unverändert und liefert weiterhin finale Texte;
interne Streaming-Partials sind keine ausgelieferten Wyoming-Partials.

Erfolgreiche technische Tests sind keine pauschale N100-Geschwindigkeitsempfehlung.
Nemotron benötigt erheblich mehr RAM und Rechenzeit als Kroko; der vollständige
lokale Vergleich wird getrennt ausgewertet. Keine Nemotron-Gewichte werden im
Repository oder Image mitgeliefert. Andere NeMo-/Parakeet-Exporte werden durch
dieses gezielte Profil nicht automatisch als kompatibel freigegeben.

## CLI und sichere Entfernung

Die Variablen des Containers gelten auch für die CLI:

```sh
docker exec YOUR_CONTAINER /usr/local/bin/model-manager list
docker exec YOUR_CONTAINER /usr/local/bin/model-manager status
docker exec YOUR_CONTAINER /usr/local/bin/model-manager verify community-128
docker exec YOUR_CONTAINER /usr/local/bin/model-manager install community-128
```

`list`, `status`, `verify` verändern nichts und laden nichts herunter.
`install` erlaubt ausdrücklich die Installation des gewählten Modells; es lädt
kein Custom-Modell aus einer Manifest-URL. Für eine andere Sprache in einem
separaten Verwaltungsaufruf `LANGUAGE` explizit setzen. `source` im Manifest ist
nur Herkunftsinformation. `runtime_compatible` ist deklarativ; erst ein nativer
Serverstart bestätigt tatsächliche Inferenz-Kompatibilität.

`model-manager remove community-128 --yes` entfernt ausschließlich das gewählte
Modell **der konfigurierten Sprache**, wenn es nicht aktiv verwendet wird und
keine unbekannten Dateien enthält. Andere Sprachen, Modelle und Appdata bleiben
erhalten. Keine pauschalen Löschbefehle. Flache gemeinsame Classic-Ablagen und
neue manuelle Custom-Paket-Caches werden konservativ nicht automatisch entfernt.
Ein entferntes Community-Modell wird bei weiterhin gewählter Variante und aktivem
Auto-Download später erneut installiert; vorher die Auswahl bewusst ändern.

## Namen und Lizenz

Der neutrale Projekt- und neue Containername ist `kroko-stt-wyoming`.
Kanonisches Repository, GHCR-Image und neue CA-Vorlage heißen ab v0.3.1
`kroko-stt-wyoming`. Neue Appdata-Verzeichnisse verwenden diesen Namen.
Bestehende Container, Appdata-Mounts und gespeicherte Unraid-Templates werden
nicht automatisch umbenannt oder geändert. Vorhandene deutsche Modelle werden
weiter ohne Download und ohne erzwungene Migration wiederverwendet.
Ein bisheriger Hostpfad mit `kroko-stt-german-wyoming` bleibt gültig.

Community: CC-BY-SA, Versionsnummer vom Anbieter nicht spezifiziert.
Classic-Rechte bleiben ungeklärt.
Gewichte sind weder im Git-Repository noch im Containerimage enthalten.
[Details](licensing.md).

### Bestehende Unraid-Installation auf den neuen Image-Namen umstellen

Optional nur die Image-Referenz auf `ghcr.io/marco-taylor/kroko-stt-wyoming:0.3.1`
ändern. Den vorhandenen Host-Modellpfad, die Sprache, Modellauswahl und das
Host-Port-Mapping übernehmen. Ein alter Hostpfad muss nicht umbenannt werden.
Die gespeicherte Benutzer-Vorlage wird nicht automatisch umgeschrieben.
Die bisherigen versionierten GHCR-Images bleiben verfügbar; ein Repository-Redirect
bei GitHub benennt ein GHCR-Paket nicht automatisch um.
