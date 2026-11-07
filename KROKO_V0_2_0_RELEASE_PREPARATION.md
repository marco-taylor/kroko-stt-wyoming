# Kroko v0.2.0: Release-Vorbereitung

> **Historisches Dokument:** Dieser Bericht beschreibt den damaligen Stand vor
> v0.2.0. Genannte frühere Template- und lokale Image-Namen sind historische
> Belege. Für aktuelle Installation und Updates gilt die [./README.md](README.md).

Status: **technisches GO, vollständig zur Veröffentlichung vorbereitet**. Ausschließlich lokal vorbereitet; nicht committet,
getaggt, gepusht oder veröffentlicht. Keine produktive Übernahme.

## Version und Historie

Vorgeschlagene nächste Version: **v0.2.0**. Lokal sowie read-only am Git-Remote
wurde bisher ausschließlich der Release-Tag v0.1.0 festgestellt. Die Git-Basis
bleibt `b84a306514f678160da4ae11f6263d693e140474`; alle Änderungen sind uncommittet.

Der Model Manager erweitert die Installation wesentlich. Ein Minor-Schritt in der
0.x-Reihe passt dazu; vorhandene Classic-Konfigurationen bleiben kompatibel.
Paketversion, Lockfile und OCI-Image-Version sind lokal auf 0.2.0 vorbereitet.
Der bestehende v0.1.0-Tag, Release und öffentliche Image-Tag wurden nicht verändert.

## Vollständige Änderungsliste gegenüber der Git-Basis

| Datei | Änderung |
| --- | --- |
| [./Cargo.toml](Cargo.toml) | Version 0.2.0, CLI-Binary, direkte Nutzung von serde und ureq |
| [./Cargo.lock](Cargo.lock) | Rootpaket 0.2.0, direkte Abhängigkeiten; keine Paketversions-Upgrades |
| [./Dockerfile](Dockerfile) | CLI einschließen, dynamische lokale Modellauswahl, Auto-Download, längere Erststart-Healthcheck-Frist, OCI-Version 0.2.0 |
| [./NOTICE](NOTICE) | Aktuelle Releasezuordnung bei unverändertem nativen Build und Lizenzumfang |
| [./.dockerignore](.dockerignore) | Neue Quellmodule im eng begrenzten Build-Kontext zulassen |
| [./.gitignore](.gitignore) | Entwicklungscaches und private lokale Analyse-/Abnahmeberichte vom öffentlichen Dateisatz ausschließen |
| [src/server/model_manager.rs](src/server/model_manager.rs) | Lokale Auswahl/Validierung, Manifest, Originaldownload, Staging, SHA256, atomare Installation, sichere Entfernung, Leases und Tests |
| [src/bin/model-manager.rs](src/bin/model-manager.rs) | CLI `list`, `status`, `verify`, `install`, `remove` |
| [src/server/models.rs](src/server/models.rs) | Bekannte Profile/Prüfsummen und sichere `custom/<name>`-IDs |
| [src/server/config.rs](src/server/config.rs) | Persistente Modellwurzel, Legacy-Umgebung, Alias `MODEL`, Auto-Download-Schalter |
| [src/server/asr.rs](src/server/asr.rs) | Vorbereitetes Modell aus Manager laden, Lease über Runtime-Lebensdauer; Inferenzparameter erhalten |
| [src/server/main.rs](src/server/main.rs) | Gemeinsames Modul und generische Modellbeschreibung |
| [scripts/run-local.sh](scripts/run-local.sh) | Persistenter schreibbarer Modell-Mount; Auswahl nur bei expliziter Angabe |
| [Historische v0.2.0-Vorlage](https://github.com/marco-taylor/kroko-stt-wyoming/blob/v0.2.0/templates/kroko-stt-german-wyoming.xml) | Community-128 für Neuinstallation, deutsche Installation-/Upgrade-Hinweise, persistenter Mount |
| [./ca_profile.xml](ca_profile.xml) | Aktualisierte CA-Beschreibung für v0.2.0 |
| [./README.md](README.md) | Einfacher Erststart, Classic-Upgrade, Auswahlmatrix, sichere CLI-Aufrufe und Release-Notes-Link |
| [docs/models.md](docs/models.md) | Automatische/manuelle/offline Installation, Custom, Rechte, Status, gezielte Entfernung und Lizenzhinweis |
| [docs/licensing.md](docs/licensing.md) | Community-Attribution ohne Lizenzversionsannahme, Model-Manager-Bezugsstrategie und HTTPS-Abhängigkeiten |
| [docs/dependency-licenses.md](docs/dependency-licenses.md) | Native Konfiguration als unverändert seit v0.1.0 einordnen |
| [docs/development.md](docs/development.md) | Reproduzierbare isolierte Model-Manager-Abnahme und aktueller Testimage-Name |
| [docs/examples/custom-model.json](docs/examples/custom-model.json) | Einfaches lokales Custom-Manifest |
| [tests/model-manager-smoke.py](tests/model-manager-smoke.py) | Isolierte A–J-Abnahme plus Classic-Upgrade trotz vorhandenem Community-128 |
| [docs/releases/v0.2.0.md](docs/releases/v0.2.0.md) | Release-Notes-Entwurf |
| Dieser Bericht | Version, Änderungen, Abnahme, Grenzen und Veröffentlichungsempfehlung |

Die bisherigen privaten Analyse- und Abnahmeberichte bleiben unverändert lokal
vorhanden. Sie enthalten interne Pfade und private Beleglinks und werden nicht
als öffentliche Release-Dokumentation eingeplant. Sie waren nie getrackt und
wurden nicht gelöscht. Öffentliche Dokumentation verweist ausschließlich auf
Dateien im vorgesehenen öffentlichen Dateisatz.

## Default-Prüfung und Classic-Upgrade

| Stelle | Geprüftes Verhalten |
| --- | --- |
| Dockerfile / Image-Umgebung | Kein festes `MODEL`/`KROKO_MODEL`, das lokale Classic-Auswahl verdrängen könnte; Auto-Download standardmäßig erlaubt |
| Entrypoint / Config | Manager-Auswahl vor Recognizer-Laden, bisherige Umgebungsvariable erhalten |
| Leere Modellwurzel ohne Auswahl | Community-128 |
| Explizites `KROKO_MODEL=classic` | Classic, auch wenn daneben Community-128 vorhanden ist |
| Vorhandenes Classic ohne explizite Auswahl | Classic zuerst, inklusive flacher und historisch benannter Ablage |
| Neue CA-Konfiguration | Explizit Community-128, persistenter Modell-Mount |
| README / Modellanleitung / Manifest-Beispiel | Neuinstallation und Upgrade getrennt, Custom lokal |
| Compose | Kein bestehendes Compose- oder Entrypoint-Shellskript im Projekt; keine unnötige zusätzliche Konfiguration angelegt |
| Tests | Leerer Root, ausdrückliche Auswahl, Koexistenz und Legacy-Ablagen geprüft |

**Bestehende Classic-Installationen funktionieren weiter. Keine automatische
Migration, kein Modellersatz und kein erforderlicher Community-Download.**
Gespeicherte Auswahl und Modellvolume beim Update behalten. Beim manuellen
Neuanlegen anhand einer neuen CA-Vorlage die bisherige Auswahl `classic`
übernehmen, denn die Neuinstallationsvorlage setzt ausdrücklich Community-128.

Community-64 bleibt optional automatisch/lokal. Classic und Custom bleiben
lokal-only. Nicht vorhandenes Classic führt zur verständlichen Dateianforderung,
nicht zum Netzwerkzugriff. Ungültige Bestandsdateien werden nicht ersetzt.

## CA und Benutzerdokumentation

Neuinstallation: Community Apps installieren, Community-128 belassen, Container
starten, automatische Einrichtung abwarten, Wyoming Protocol in Home Assistant
mit Host und Port verbinden. Der dokumentierte öffentliche Ablauf setzt das
später veröffentlichte v0.2.0-Image voraus; das heutige v0.1.0 bleibt unverändert.

Die Vorlage hat Community-128 als Default, deutschen Hinweis auf einmaligen
Originaldownload, SHA256, persistente Speicherung und downloadfreie Neustarts.
Classic wird als lokale Alternative genannt, Custom als fortgeschrittene Option.
Keine zusätzlichen Release-Vorbereitungsoptionen wurden eingeführt. Der bereits
implementierte Auto-Download-Schalter bleibt eine erweiterte Option.

Die Modellverwaltung wird über `docker exec` vom Unraid-Terminal dokumentiert;
das Distroless-Image enthält keine Shell. `remove` ist auf ein einzelnes Profil
begrenzt, benötigt ausdrückliches `--yes` und verweigert aktive Modelle sowie
fremde Dateien. Keine pauschalen Löschbefehle oder Appdata-Bereinigung.

Modelle liegen persistent außerhalb des Images. Ein Containerupdate führt weder
zur Löschung noch zum erneuten Download vollständiger gültiger Modelle.
Originale Community-Pakete bleiben erhalten. Die bestehende sherpa-onnx-Anbindung
benötigt vier Dateien; sie werden einmalig bytegenau bereitgestellt und danach
wiederverwendet. Keine neue ASR-Runtime oder Graphkonvertierung.

## Erneut ausgeführte Tests

| Prüfung | Ergebnis |
| --- | --- |
| Rust `cargo test --locked` | 15 verschiedene Tests; 30 Ausführungen (15 + 13 + 2), alle bestanden; tatsächlich neu ausgeführt |
| `cargo build --release --locked --bins` | Erfolgreich im Dockerfile |
| Docker-Build des endgültigen Quellenstands | Erfolgreich, lokales Testimage, kein Registry-Push |
| A: leerer Root / automatischer Originaldownload | Bestanden, Größe/SHA256 und native Spracherkennung |
| B/C: Bestand / tatsächlicher Neustart | Kein Download, read-only/offline, Dateihashes unverändert |
| D: Classic und beide Legacy-Formen | Bestanden, native Spracherkennung ohne Netzwerk |
| Zusätzlicher Classic-Upgrade-Fall | Explizit Classic trotz vollständig vorhandenem Community-128; korrekte Transkripte, keine Dateiänderung |
| E: Classic fehlt | Verständliche lokale Dateianforderung, kein Download |
| F: Teilmodell / ungültiger Hash | Bestehende Datei unverändert, kein Ersatz |
| G: manuelles Originalpaket | Offline vorbereitet und nativ geladen; Originalhash erhalten |
| H: Custom-Manifest | Lokale validierte Fixture geladen; keine neue Runtime eingeführt |
| I: gezieltes Entfernen | Aktive Modelle/fremde Dateien geschützt, andere Modelle und Konfiguration erhalten |
| J: Download hart abgebrochen / abgeschnittenes Paket | Kein beschädigtes Modell als installiert erkannt |
| Tatsächlich installierte Auto-/manuelle Cache-Form entfernen | Beide bestanden, Classic und Konfigurationsdatei daneben SHA-identisch |
| Reale Unraid-Abnahme mit v0.2.0 | Neuinstallation, Host-Port, Stop/Start, read-only und vollständig netzlos bestanden |
| Image-Audit | Keine Gewichte/Audios/Python-Executable; drei Anwendungsbinaries; aktuelle Lizenzhinweise byteidentisch zum Quellstand |
| XML / JSON / Shellsyntax | Gültig; Community-128-Default und persistenter Mount geprüft |
| Markdown / lokale Dateilinks | Abschließend geprüft, keine Verweise auf ausgeschlossene private Dateien |
| Öffentlicher Dateisatz | Keine Gewichte, Secrets, temporären Testdaten oder internen absoluten Hostpfade |
| Bestandsschutz | Produktiver Container, Image, Ports, Konfiguration, Appdata und vorhandene Modell-/Benchmarkdateien identisch |

Endgültiges lokales Testimage: `kroko-stt-german-wyoming:v0.2.0-test`,
Image-ID `sha256:509cd99b9f7987ee7c36bf30418ee0c85244edfa72f96b1a36151cdfba8d408a`,
unkomprimiert **90.761.849 Bytes**.
Alle Modelltests nacheinander, CPU only, ein Inferenzthread, UID/GID 99:100.

Erneute reale Abnahme auf Intel N100 / Unraid 7.3.4:
Host-Port `127.0.0.1:58635`, eigener Container; produktiver Hostport 10321
unberührt. Describe bestätigte Programmversion 0.2.0 und Community-128.

| Ablauf | Tatsächliches Transkript | Request-Wallzeit | Wall-RTF |
| --- | --- | ---: | ---: |
| Erststart / Host-Port | Einsteigen bitte | 282.8 ms | 0.1885 |
| Stop/Start / Host-Port | Einsteigen bitte | 264.3 ms | 0.1762 |
| Read-only / Host-Port | Einsteigen bitte | 271.8 ms | 0.1811 |
| Offline + read-only / Namespace | Einsteigen bitte | 275.1 ms | 0.1834 |

Referenzaufnahme „Einsteigen bitte!“, PCM16 mono 16 kHz, 1,50025 s.
Schnelle Eingabe in 20-ms-Blöcken; Wall-RTF enthält Transport, Vorbereitung und
Tail, ist keine reine Inferenz-RTF und kein Ersatz für einen Qualitätsbenchmark.
Im Offline-Test war ausschließlich der eigene Container netzlos; der Testclient
trat nur in dessen Netzwerk-Namespace ein. Keine Hostnetzänderungen.
Ein Download beim Erststart, kein zweiter Download, alle installierten Dateihashes,
Größen, Besitzer, Rechte und Änderungszeiten bei Folgestarts identisch.

Die erzeugten Testcontainer und ausschließlich für diese Tests angelegten
Modellkopien wurden wieder entfernt. Lokale Belege, Quellbackup und Testimage
bleiben außerhalb des öffentlichen Dateisatzes erhalten. Die Schutzprüfung
verglich 45 bestehende Dateihashes und 16 produktive Appdata-Einträge.

Der Testsatz verwendet ausschließlich eigene Container, neue Testverzeichnisse
und kopierte lokale Fixtures. Keine produktiven Ports oder Modellmounts.
Es wurden keine Benchmarks neu ausgewertet oder Ergebnisse verändert.

## Lizenz- und Artefaktprüfung

- Community-Modell: **CC-BY-SA, upstream version unspecified**.
- Attribution in [docs/licensing.md](docs/licensing.md) unverändert erhalten;
  keine konkrete CC-Version im öffentlichen Dateisatz behauptet.
- Originalanbieter Banafo / Kroko ASR, Hugging-Face-Quelle und gepinnte SHA256.
- Gewichte werden nur direkt vom Originalanbieter auf Anforderung installiert,
  nicht in Git, Docker-Image, Release-Assets oder einem Mirror verteilt.
- Classic-Gewichtslizenz bleibt ungeklärt; deshalb kein Classic-Auto-Download.
- Eigenes Projekt Apache-2.0; Drittanbieterhinweise und entsprechende
  glibc-/Eigen-Quellen weiterhin separat im Image. Keine Um-Lizenzierung.
- Kein Abhängigkeitsversions-Upgrade: zusätzliche direkte Nutzung bereits im
  Lockfile vorhandener serde-/ureq-Pakete; native Runtime unverändert.
- Öffentlicher Dateisatz und Image separat auf Modell-/Audio-Dateien geprüft;
  keine Secrets, temporären Testdaten oder internen absoluten Hostpfade im
  vorgesehenen öffentlichen Dateisatz. Allgemeine dokumentierte Containerpfade,
  Unraid-Appdata-Standard und Platzhalter sind keine privaten Hostpfade.

Die genaue Community-Lizenzversion bleibt die dokumentierte Upstream-Unklarheit.
Diese Vorbereitung behauptet keine weitergehende Freigabe zur Modellverteilung.

## Release-Notes-Entwurf

Vollständiger Entwurf: [v0.2.0](docs/releases/v0.2.0.md).

Er hebt Model Manager, Community-128 für Neuinstallation, direkten
Originaldownload, SHA256, atomare Bereitstellung, persistente/offlinefähige
Installation, Classic-Abwärtskompatibilität, Custom-Manifeste und UID/GID 99:100
hervor. Upgrade-Anleitung und Grenzen stehen ausdrücklich dabei. Nicht als
GitHub Release angelegt.

## GO / NO-GO

**GO für den vorbereiteten technischen und dokumentarischen v0.2.0-Stand.**
Alle geforderten Regressionstests sind bestanden; keine technische Nacharbeit
steht aus. Community-128 ist der Neuinstallationsstandard, Classic bleibt
abwärtskompatibel. Die dokumentierte unbestimmte Community-Lizenzversion ist
keine technische Testlücke und wird nicht durch eine erfundene Version ersetzt.
Diese Empfehlung ist keine zusätzliche Freigabe zur Weiterverteilung von Gewichten.

Alle tatsächlichen Veröffentlichungsschritte bleiben offen und wurden nicht
angefordert: Commit, Tag v0.2.0, Push, Registry-Push, GitHub Release und
CA-Veröffentlichung. Der produktive Container wurde weder gestoppt noch
neu erstellt oder konfiguriert. Keine Versionsänderung an einem bereits
veröffentlichten Artefakt.
