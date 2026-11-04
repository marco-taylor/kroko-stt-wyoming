# Modelle installieren und verwalten

## Community-128 automatisch

Für eine Neuinstallation **Community-128 auswählen**, einen persistenten Appdata-
Modellordner nach `/models` **schreibbar** einbinden und den Container starten.
Fehlt das Modell, lädt der Rust Model Manager das offizielle Banafo-Paket,
prüft die feste SHA256 und bereitet es einmalig für sherpa-onnx vor.
Danach startet Wyoming. Beim nächsten Start wird das vorhandene Modell geprüft
und ohne Download verwendet. Es werden keine Gewichte im Image ausgeliefert.

Diese Anleitung beschreibt v0.2.0. Das öffentliche Image ist unter
`ghcr.io/marco-taylor/kroko-stt-german-wyoming:0.2.0` sowie `:latest` verfügbar.
Eine bestehende v0.1.0-Installation erhält die neue Funktion durch ein
Image-Update mit beibehaltener Konfiguration, nicht durch einen bloßen Neustart.

Community-128 erreichte im separaten 100-Aufnahmen-/Drei-Wiederholungen-Vergleich
7,28 % WER, Classic 7,85 %. Der kleine Punktwertvorteil ist keine Garantie für
andere Aufnahmen; Community-128 benötigt mehr CPU/RAM und besitzt einen internen
Schritt von 2,56 s statt 1,28 s. Das ist der bewusst gewählte Qualitätsstandard
für **Neuinstallationen**, kein automatischer Wechsel vorhandener Classic-Nutzer.

## Auswahl und Verzeichnisse

`MODEL_DIR` bezeichnet weiterhin den übergeordneten Modellordner.
`KROKO_MODEL` bleibt erhalten; alternativ ist `MODEL` möglich, beispielsweise
`MODEL=custom/my-german-model`. Bei widersprüchlichen Werten startet der Server
nicht. Ein Modellwechsel erfordert eine geänderte Containerkonfiguration,
nicht nur `docker restart`.

```text
/models/
  community-128/
    Kroko-DE-Community-128-L-Streaming-001.data
    encoder.onnx
    decoder.onnx
    joiner.onnx
    tokens.txt
    installation.json
  community-64/
  classic/
  custom/
    my-german-model/
      model.json
      encoder.onnx
      decoder.onnx
      joiner.onnx
      tokens.txt
```

Ohne explizite Auswahl wird ein vorhandener Classic-Ordner zuerst erkannt, damit
bestehende Installationen ihren bisherigen Standard behalten. Danach kommen
vorhandenes Community-128 und Community-64. Ist nichts vorhanden, gilt
Community-128. Auch unvollständiges Classic wird nicht durch einen stillen
Download eines anderen Modells umgangen. Das neue Unraid-Template wählt
Community-128 ausdrücklich; bestehende gespeicherte Templates mit
`KROKO_MODEL=classic` behalten ihre Auswahl.

## Was bei jedem Start passiert

1. Lokales Modell suchen, einschließlich unterstützter Classic-Legacypfade.
2. Alle erforderlichen Dateien prüfen: vorhanden, regulär, nicht leer, erlaubte
   Größe, bekannte SHA256; zusätzlich ONNX-Protobuf-Hülle und Tokenformat.
3. Vollständiges Modell unverändert verwenden. Keine Downloads, Updates,
   Markierungsdateien oder Rechteänderungen bei diesem normalen lokalen Start.
4. Lokales gültiges Community-Paket bei Bedarf einmalig bytegenau bereitstellen.
5. Nur ein fehlendes Community-Modell bei `MODEL_AUTO_DOWNLOAD=true` herunterladen.
6. sherpa-onnx lädt die Graphen und prüft ihre tatsächliche Runtime-Kompatibilität,
   bevor der TCP-Listener startet.

Ungültige oder teilweise vorhandene Komponenten führen zu einer Fehlermeldung
mit Ursache und Pfad. Es gibt kein automatisches Löschen, Ersetzen oder Reparieren.
Eine Prüfsumme identifiziert die ausgewählte Revision; sie erteilt keine Lizenz.

## Originalformat und einmalige Vorbereitung

Die bestehende sherpa-C-API erwartet vier Dateien und unterstützt die offiziellen
Kroko-Pakete nicht direkt. Deshalb behält der Manager das originale `.data`-Paket
**unverändert** und kopiert dessen vier Komponenten einmalig bytegenau heraus.
Keine ONNX-Konvertierung, Quantisierung oder neue Inferenz-Runtime.

Bei einer automatischen Neuinstallation werden Paket und Komponenten zusammen
im neuen Modellordner atomar bereitgestellt. Wer nur ein lokales `.data`-Paket
kopiert, erhält die Komponenten unter `.runtime/<Paket-SHA256>/` innerhalb dieses
Modellordners. Auch ein zunächst leerer Modell-Unterordner kann diese Cacheform
verwenden. Beim Neustart wird der Cache geprüft und wiederverwendet.

Bereits vorhandene vollständige ONNX-/Token-Komponenten haben Vorrang und brauchen
weder Paket noch Cache. Ein zusätzliches unbenutztes `.data`-Paket wird dann nicht
zur Voraussetzung gemacht. Unvollständige Komponenten werden nicht aus einem
Paket ergänzt: zuerst den Konflikt prüfen und bewusst beheben.

## Community-64 optional

`KROKO_MODEL=community-64` setzen. Bei fehlendem Modell gilt derselbe sichere
Originaldownload wie für Community-128. Alternativ das originale Paket
`Kroko-DE-Community-64-L-Streaming-001.data` lokal im entsprechenden Modellordner
bereitstellen. Nur das ausgewählte Modell wird geladen.

## Classic manuell

`KROKO_MODEL=classic` setzen und die vorhandenen, passenden vier Dateien in
`/models/classic/` verwenden. **Classic wird niemals automatisch heruntergeladen.**
Fehlt es, nennt die Fehlermeldung die benötigten Dateien und den Zielordner.

Bereits funktionierende Pfade bleiben unterstützt:

- `/models/classic/` mit den vier bisherigen Dateien;
- `/models/sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06/`;
- die vier Dateien direkt unter dem als `MODEL_DIR` konfigurierten Verzeichnis.

Keine Verschiebung oder Umbenennung ist für diese bestehenden Exporte nötig.
Die SHA256 des bekannten Classic-Exports bleiben unverändert in
[src/server/models.rs](../src/server/models.rs).
Andere eigene Exporte gehören unter eine Custom-ID, nicht unter `classic`.
Die Classic-Gewichtslizenz bleibt ungeklärt; keine freie Weiterverteilung oder
Lizenzfreigabe durch dieses Projekt.
Der [historische Originalexport](https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06/tree/887db3d083240198c2d2b99fb66cfcfe6948ced8)
bleibt als Herkunft dokumentiert; ein Link erteilt keine Nutzungsrechte.

## Offline und manuelle Installation

Vorhandene vollständige Modelle werden ohne Netzwerk verwendet, auch wenn der
Download grundsätzlich aktiviert ist. Mit `MODEL_AUTO_DOWNLOAD=false` wird auch
bei fehlendem Modell kein automatischer Netzwerkzugriff versucht.

Entweder die vier passenden Komponenten kopieren oder das offizielle Community-
Paket im jeweiligen Modellordner ablegen. Für ein reines Paket braucht die
**erste** Vorbereitung dort Schreibrechte. Bereits vollständige Komponenten und
fertige Caches funktionieren auch mit einem read-only Modellmount.
Symlinks innerhalb der Modellpfade werden aus Sicherheitsgründen abgewiesen;
reale Dateien oder ein Volume-/Bind-Mount verwenden. Das explizit konfigurierte
Wurzelverzeichnis darf selbst auf persistenten Storage zeigen.

## Custom-Modelle

Eigene Modelle liegen unter `/models/custom/<name>/`; Namen dürfen ASCII-Buchstaben,
Ziffern, Bindestrich und Unterstrich enthalten. Dort eine lokale `model.json`
und die darin genannten Dateien ablegen. Ausgangspunkt:
[docs/examples/custom-model.json](examples/custom-model.json).

Das einfache Schema enthält ID, Anzeigename, Sprache, Modelltyp, Format, Backend,
Version, Lizenz, vier Rollen mit relativen Pfaden und optionalen SHA256 sowie
`tail_samples`. Für echte Integritätskontrolle die Hashes eintragen; `null` prüft
nur Existenz und Format. Pfadtraversal, absolute Dateipfade, doppelte Rollen,
Symlinks und reservierte Dateinamen werden abgewiesen.

Die vorhandene Runtime kann `zipformer`-/`zipformer2`-Streaming-Transducer mit
80 Filterbankkanälen, 16 kHz, Blank-ID 0 und den vier Rollen `encoder`, `decoder`,
`joiner`, `tokens` laden. Der tatsächliche ONNX-Graph muss dazu passen.
`tail_samples` muss zum Export passen und ein Vielfaches von 320 zwischen 320
und 160000 sein; der Beispielwert ist **keine** allgemeine Modellgarantie.
Decoder und CPU-Einstellungen bleiben unverändert: Greedy, standardmäßig ein
Inferenzthread. Eine neue ASR-Runtime wird nicht installiert.

Andere deklarierte Backends/Formate können bereits als lokale Manifest-/Dateisätze
geprüft werden. Der Server weist sie mit einer klaren Meldung als nicht unterstützt
zurück. `runtime_compatible` in der CLI ist eine deklarative Einordnung, kein
vorgezogener nativer Graph-Ladetest. Nur der Serverstart bestätigt das tatsächliche
Laden. Custom-Modelle werden niemals automatisch aus einer Manifest-URL geladen;
`source` dokumentiert lediglich die Herkunft.

## Prüfen und Status

Die CLI liegt im Image. Vom Unraid-/Docker-Host aus aufrufen;
`YOUR_KROKO_CONTAINER` durch den eigenen Containernamen ersetzen.
Das Distroless-Image enthält keine Shell und benötigt keine Container-Konsole:

```sh
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager list
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager status
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager verify community-128
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager verify classic
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager verify custom/my-german-model
```

Die Ausgabe ist JSON mit Auswahl, vorhanden/gültig, Quelle `local`/`download`,
Format, Version/Hashes, Runtime-Einordnung und Fehlerursache. `list`, `status`
und `verify` verändern keine Modell-Dateien und benötigen kein Internet.
`verify` beendet sich bei Fehler mit Exitcode 1. Ein gültiges lokales Paket meldet
`needs_preparation=true`, solange sein Cache fehlt. Die nativen Graphen werden
nicht heimlich für Diagnosebefehle geladen.

Für explizite Installation außerhalb des Serverstarts, bei laufendem Container:

```sh
docker exec YOUR_KROKO_CONTAINER /usr/local/bin/model-manager install community-128
```

Dieser ausdrücklich aufgerufene Installationsbefehl darf herunterladen; der
Umgebungsschalter steuert den **automatischen Serverstart**, nicht diesen Befehl.
Auch `install` ersetzt keine vorhandenen ungültigen Dateien.

## Genau ein Modell entfernen

Vorher den Server stoppen, der **dieses** Modell verwendet. Dann beispielsweise
in einem separaten Container mit demselben Modellvolume ausführen:

```sh
model-manager remove community-128 --yes
```

Beispiel für ein separat gestartetes Verwaltungsprogramm unter Unraid, nachdem
alle Container mit diesem Modell gestoppt wurden (Pfad/Image bei Bedarf anpassen):

```sh
docker run --rm --network none --read-only --user=99:100 \
  --cap-drop=ALL --security-opt=no-new-privileges \
  --mount type=bind,src=/mnt/user/appdata/kroko-stt-german-wyoming/models,dst=/models \
  --entrypoint /usr/local/bin/model-manager \
  kroko-stt-german-wyoming:local remove community-128 --yes
```

Für `list` oder `verify` genügt derselbe Aufruf mit geändertem Befehl; dafür muss
der Server nicht gestoppt werden, und der Mount kann zusätzlich `readonly` sein.

Nur das angegebene dedizierte Modellverzeichnis ist betroffen. Eine aktive
Modellsperre verhindert das Entfernen. Enthält der Ordner unbekannte Dateien,
Konfiguration oder Symlinks, wird vor der ersten Löschung abgebrochen.
Geschwistermodelle, Appdata-Konfiguration und Wyoming-Daten bleiben erhalten.
Die gemeinsame flache Classic-Legacyablage wird absichtlich nicht pauschal
entfernt; ihre vier Dateien zuvor bei Bedarf in einen dedizierten Classic-Ordner
verschieben. Es gibt kein globales `purge` und keine Force-Reparatur.
Bleibt das entfernte Community-Modell ausgewählt und Auto-Download aktiv, wird es
beim nächsten Start wieder installiert. Vorher ein anderes Modell auswählen oder
`MODEL_AUTO_DOWNLOAD=false` setzen. Der Entfernungsbefehl ändert keine Konfiguration.

## Download, Updates und Abbrüche

Die einzige automatische Quelle ist das gepinnte offizielle
[Banafo-Repository](https://huggingface.co/Banafo/Kroko-ASR/tree/d45212aeb212dd66083dd22710c9954f40ff8cc1).
HTTPS bleibt auch bei Weiterleitungen erforderlich; der Anbieter kann seine
Hosting-CDNs verwenden. Keine Projekt-Mirrors und kein automatisches `main`-Update.
Der Download ist in Größe und Zeit begrenzt. Paket-SHA, Sprach-/Formatheader,
Blockgrenzen und alle vier Komponenten-SHA werden vor Veröffentlichung geprüft.

Neue Pakete liegen zuerst in einem privaten Staging-Verzeichnis auf demselben
Volume. Erst das vollständig geprüfte Verzeichnis wird mit Linux
`renameat2(RENAME_NOREPLACE)` atomar veröffentlicht. Auch ein gleichzeitig
erschienener Zielordner wird nicht überschrieben. Die Funktion erfordert ein
Linux-Dateisystem mit dieser Operation; Unraid XFS/Btrfs sind vorgesehen.
Normale Downloadfehler entfernen nur das eigene Staging-Verzeichnis. Ein harter
Prozess-/Hostabbruch kann ein verborgenes Staging-Verzeichnis hinterlassen;
dieses zählt niemals als installiertes Modell und wird nicht global gelöscht.
Ein abgebrochener Download wird nicht als fertiges Paket weiterverwendet.

Container-/Image-Updates löschen keine Modelle: das gesamte Modellverzeichnis
bleibt im persistenten Appdata-Mount. Für lokale vorhandene Modelle reicht `ro`;
für automatische Installation, Paketvorbereitung und `remove` ist `rw` nötig.
Der Container selbst bleibt unprivilegiert, UID 99 / GID 100, mit read-only RootFS.
Ein vorhandener Modellordner wird nicht automatisch umberechtigt. Fehlen Rechte,
nur den vorgesehenen Modellordner für UID 99/GID 100 schreibbar machen, keine
pauschale Appdata-Reparatur und kein `chmod 777`.

## Lizenz und Attribution

Die offiziellen Quellen nennen Community-Modelle CC-BY-SA, weiterhin ohne
nachgewiesene genaue Versionsangabe. Diese Restunsicherheit bleibt sichtbar;
Es wird keine konkrete CC-BY-SA-Version unterstellt. Herkunft, Revision, Hash und Lizenzhinweis
bleiben bei automatischen Installationen in `installation.json` erhalten.
Der Download ist ein direkter Bezug vom Anbieter, keine Modellverteilung im
Image. Classic bleibt lokal-only. Einzelheiten:
[docs/licensing.md](licensing.md).

Der Hinweis für Community-128 steht unter
[Kroko Community model attribution](licensing.md#kroko-community-model-attribution).
