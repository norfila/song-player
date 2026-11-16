# Song Player

Lokaler Audio-Player für Windows, Version **0.4.3**. Tauri 2 und React bilden die
Oberfläche; Rust verwaltet Wiedergabe, Bibliothek, Playlists und Warteschlangen.
Die native Audioausgabe übernimmt libmpv. Audiodateien bleiben an ihrem
ursprünglichen Speicherort.

## Funktionen

- Dateien und Ordner öffnen, Ordner zur Bibliothek hinzufügen und Dateien per
  Drag-and-drop in die Warteschlange übernehmen.
- Play/Pause, nächster/vorheriger Titel, Spulen, Lautstärke und Stummschaltung.
  Über dem Lautstärkeregler, Symbol oder Prozentwert ändert das Mausrad die
  Lautstärke; das gilt im unteren Player und in der großen Wiedergabeansicht.
- Integrierte Titelleiste mit Datei-/Wiedergabe-/Ansichtsmenü und Fensterknöpfen.
- Große Wiedergabeansicht mit Cover und Audiosteuerungen. „Zurück“ startet nach
  mehr als drei Sekunden den aktuellen Titel neu; ein eigener Stop-/Resetknopf entfällt.
- Warteschlange bearbeiten, Einträge umsortieren, als Nächstes vormerken oder
  anhängen; denselben Titel mehrfach aufnehmen.
- Einzelnen Titel oder ganze Warteschlange über ein Auswahlmenü wiederholen.
  Zufallswiedergabe für die aktive Warteschlange ein- und ausschalten; beim
  Ausschalten wird die vorherige Abspielreihenfolge wiederhergestellt.
- Nach Titel, Interpret, Album, Dateiname, Jahr, Dauer oder Importzeit sortieren;
  die Auswahl zeigt die Richtung ausdrücklich an, etwa „Titel · A–Z“.
- Bibliothek durchsuchen und nach Alben oder Ordnern anzeigen; Musikordner im
  Hintergrund erneut einlesen, wenn sich Dateien ändern.
- Ordner direkt in der Ordneransicht aus der Bibliothek entfernen (Symbol auf der
  Ordnerkarte oder Knopf in der geöffneten Ordneransicht) oder unter
  Einstellungen → Bibliothek. Die Dateien bleiben erhalten. Entfernte Unterordner
  eines Musikordners bleiben auch beim erneuten Einlesen draußen, bis sie wieder
  hinzugefügt werden; einzeln hinzugefügte Dateien bleiben in der Bibliothek.
- Leere Wiedergabelisten erstellen oder eine Warteschlange ausdrücklich als Liste
  speichern. Ein durchsuchbarer Dialog bearbeitet die Mitgliedschaft in mehreren
  Listen. Angeheftete Listen erscheinen in der Seitenleiste; jede Liste kann ein
  eigenes Symbol oder ein importiertes Bild erhalten. M3U/M3U8-Import und M3U8-Export.
- Titel als Favoriten markieren und in einer eigenen Ansicht wiederfinden.
- Mehrere Warteschlangen mit eigener Position und Abspielreihenfolge; jeweils eine spielt ab.
- Metadaten, eingebettete Cover und Coverdateien im Musikordner anzeigen.
- Lautstärke zwischen Titeln angleichen: ReplayGain-Werte analysieren und lokal
  speichern, einzeln oder gemeinsam für Alben und Ordner. Optional kompatible Tags
  in unterstützte Dateien schreiben. Dynamischer Ausgleich bleibt eine separate
  erweiterte Einstellung. Fünfband-Equalizer mit Vorverstärkung und Presets.
- Einschlaftimer im Menü der aktiven Warteschlange und in der Wiedergabeansicht:
  nach Zeit, nach dem Titel oder am Ende der Warteschlange pausieren.
- Hörstatistik für Monat, Jahr und Gesamtzeit mit kompakten Datumsfeldern und
  ausschließlich Jahren mit Hörverlauf. Erfassung, Zählgrenze, Export und Löschen
  liegen unter Einstellungen → Statistik; die Statistikseite verlinkt direkt dorthin.
- Schnellsuche mit Tippfehlertoleranz, Tastaturnavigation und lokal gelernter
  Rangfolge. `>` sucht Aktionen. Ein eigenes optionales Windows-Systemkürzel
  öffnet die Suche auch außerhalb des Appfensters, solange die App läuft.
- Einfacher Audioeditor mit Wellenform, Auswahl, Vorschau und Rückgängig:
  Auswahl behalten oder entfernen und als neue FLAC-/WAV-Datei exportieren.
- Deutsch und Englisch mit Sprachwechsel während der Wiedergabe.
- Windows-Systemmediensteuerung, Medientasten, native Menüs und Tray-Menü.
- Optional beim Schließen in den Infobereich wechseln und weiter abspielen.
- Eigene Tastenkürzel, auch für die große Wiedergabeansicht; mit Konfliktprüfung
  und Rücksetzen auf die Vorgaben.
- Breite von Bibliotheksnavigation und aktiver Warteschlange mit der Maus ändern.
  Größen werden gespeichert; Doppelklick auf den Trenner setzt sie zurück.
- Einstellungen mit Kategorien und Suche; Treffer öffnen und markieren die
  passende Einstellung. Die aktive Warteschlange lässt sich auch dort ein- und
  ausblenden; schmale Einstellungsbereiche verwenden horizontal blätterbare Kategorien.
- Designs Hell, Dunkel und Gold; eigene Designs mit Farben, Schrift, Textgröße,
  Abständen, Rundungen und Schatten. Änderungen lassen sich vor dem Speichern
  ausprobieren, übernehmen oder verwerfen.
- Designs separat als JSON importieren und exportieren. Einstellungen nach
  Bereichen sichern, vor dem Import vergleichen und den letzten Import auch
  nach einem Neustart rückgängig machen.
- Hinweis für Windows-Standardprogramme mit direktem Aufruf der Song-Player-Seite,
  „Später erinnern“ und „Nicht mehr erinnern“.
  Die Auswahl der Dateizuordnungen erfolgt in den Windows-Einstellungen.
- Dateien aus dem Explorer an die vorhandene App-Instanz übergeben;
  Dateizuordnungen sind im Installer vorgesehen. Externe Dateien können direkt
  die Wiedergabeansicht öffnen; ein vorübergehender Hinweis bietet an, den
  zugehörigen Ordner zur Bibliothek hinzuzufügen. Beides ist einstellbar.
- Bibliothek, Sitzungen und Einstellungen in SQLite wiederherstellen. Nach einem
  gewöhnlichen App-Start beginnt die Musik erst durch eine Wiedergabeaktion.
- Ruhiger Start: Das Fenster erscheint erst, wenn die erste Ansicht mit Sprache
  und Design fertig ist. Mit einem gespeicherten aktuellen Titel öffnet die App
  direkt „Jetzt läuft“.

Die früher als „Hörsitzungen“ bezeichneten Einträge heißen in der Oberfläche
„Warteschlangen“. Sie speichern zusätzlich die aktuelle Position und Misch-/Repeatmodi.
Eine Wiedergabeliste speichert dagegen eine Titelliste zum späteren Abspielen.
Pause hält die Position; „Zurück“ startet nach drei Sekunden den Titel neu.
Die große Ansicht öffnet sich über Cover, Titel oder das Vergrößern-Symbol im Player.
Der Lautstärkeausgleich lässt sich dort, im Wiedergabemenü und in den Audioeinstellungen aktivieren.
Zufallswiedergabe und Wiederholmodus verwenden im unteren Player, in der großen
Ansicht und in der aktiven Warteschlange dieselben Einstellungen. Das Wiederholmenü
bietet „Nächsten Titel spielen“, „Warteschlange wiederholen“ und „Titel wiederholen“
mit Symbolen und einer Radioauswahl. Die Abspielreihenfolge lässt sich über das
Dreipunktmenü der Warteschlange ändern. Kurze Wiedergabeaktionen lösen keinen
globalen Ladeindikator aus; andere Vorgänge zeigen ihn erst nach 300 ms.

„Beim Schließen in den Infobereich minimieren“ steht unter Einstellungen → Allgemein.
Über Datei → Beenden oder das Tray-Menü lässt sich Song Player vollständig beenden.
Tastenkürzel werden unter Einstellungen → Tastenkürzel geändert: die Zuordnung
anklicken und die neue Kombination drücken. Escape bricht die Aufnahme ab,
Rücktaste entfernt eine Zuordnung. Vorgabe für die große Ansicht ist
`Ctrl + Shift + P`; die Kürzel gelten bei fokussierter App und nicht während der Texteingabe.
`Ctrl + F` öffnet die Schnellsuche; innerhalb der Einstellungen bleibt die
Einstellungssuche direkt erreichbar. Das globale Suchkürzel wird separat unter
Einstellungen aktiviert und benötigt Ctrl oder Alt; globale Konflikte werden gemeldet.
Ein Hinweis auf fehlende Standardzuordnungen lässt sich mit „Später erinnern“ für
den aktuellen App-Start ausblenden. „Nicht mehr erinnern“ speichert die Abwahl
dauerhaft. Der direkte Aufruf der Windows-Einstellungen bleibt unter Einstellungen
→ Allgemein verfügbar. Es gibt keinen zusätzlichen Erinnerungsschalter in den Einstellungen.

Das Mausrad ändert die Lautstärke über dem Lautstärkebereich um zwei Prozentpunkte
je gewöhnlicher Rastung; feine Touchpad-Bewegungen werden gesammelt. Der Bereich
bleibt zwischen 0 und 100 Prozent, eine bestehende Stummschaltung bleibt erhalten.

Unter Einstellungen → Darstellung stehen Hell, Dunkel und Gold bereit. Ein
Vorgabedesign lässt sich duplizieren und als eigenes Design anpassen. Die Vorschau
gilt für das ganze Fenster; „Übernehmen“ speichert sie, „Verwerfen“ stellt das
gespeicherte Design wieder her. Ein Designexport enthält ausschließlich das Design
(`sonora-theme`, Version 1), keine Ordner, Tastenkürzel oder Audioeinstellungen.

Unter Einstellungen → Daten & Sicherung lassen sich Allgemein, Darstellung, Audio,
Tastenkürzel und Fensteraufteilung einzeln exportieren. Ausgabegerät und
Bibliotheksordner sind optional, weil sie vom jeweiligen Computer abhängen. Beim
Import einer Einstellungsdatei (`sonora-settings`, Version 1) werden nur ausgewählte
Bereiche übernommen; die Vorschau zeigt die Werte davor und danach. Ordner werden
ergänzt, die Musikdateien selbst bleiben an ihrem Speicherort. Bibliothekstitel,
Warteschlangen und Playlists sind kein Bestandteil dieser Einstellungsdatei.
„Letzten Import rückgängig machen“ stellt die vorherigen Einstellungen und
Ordnerzuordnungen wieder her und bleibt auch nach einem Neustart verfügbar.

Die Statistik zählt standardmäßig nach 50 Prozent eines Titels oder vier Minuten,
je nachdem, was zuerst erreicht wird. Pausen und übersprungene Zeit zählen nicht.
Jeder Abspieldurchlauf zählt höchstens einmal; Wiederholungen können erneut zählen.
Die Grenze lässt sich rückwirkend auf den gespeicherten Verlauf anwenden. Die
Zeitauswahl ordnet einen Durchlauf nach seinem Startzeitpunkt zu. Verlauf und
Suchlernen bleiben lokal und lassen sich getrennt zurücksetzen; Statistik kann
deaktiviert werden. Hörzeiten aus älteren Versionen lassen sich nicht nachträglich rekonstruieren.

ReplayGain ändert einen ganzen Titel gleichmäßig und erhält seine Dynamik.
Die Album-/Ordneroption analysiert eine Gruppe gemeinsam, sodass bewusst leise
Gruppentitel leiser bleiben. Ordnerwerte existieren ausschließlich in der App;
es gibt keinen interoperablen ReplayGain-Ordner-Tag. Tag-Schreiben ist eine
ausdrückliche Option für FLAC, MP3, Ogg Vorbis und M4A. Ohne diese Auswahl verändert
die Analyse keine Musikdateien. Der dynamische Ausgleich passt auch Passagen
innerhalb eines Titels an. Equalizer, ReplayGain und dynamischer Ausgleich
verwenden jeweils eigene Filter und entfernen keine fremden Filter.

Der Editor überschreibt die Quelldatei nicht und exportiert zunächst nur FLAC
oder WAV. Verlustbehaftete Quellen erhalten dadurch keine verlorene Qualität
zurück. Zusätzlicher Speicherplatz wird benötigt. Lange Analyse-/Exportaufgaben
laufen im Hintergrund und können abgebrochen werden.

libmpv liefert die breite Formatunterstützung, unter anderem für WAV/AIFF, FLAC,
ALAC, MP3, AAC/M4A, Vorbis, Opus, WMA, WavPack und APE. Nicht jedes Format und
jede Variante wurde bereits mit einer eigenen Testdatei geprüft. Metadaten liest
Lofty; bei einem dort unbekannten Format bleibt die Datei mit Dateinamen als
Titel abspielbar. Gapless verwendet den libmpv-Modus `weak`; lückenlose Übergänge
bei beliebigen Format-/Gerätewechseln werden nicht zugesichert.

## Unter Windows starten

Voraussetzungen für einen frischen Checkout:

- Node.js ab 22.12 und npm.
- Rust stable mit dem Ziel `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools mit **Desktop development with C++**, MSVC und einem
  Windows SDK.
- Microsoft Edge WebView2 Runtime für die Tauri-Oberfläche.

Die [Tauri-Voraussetzungen](https://v2.tauri.app/start/prerequisites/#windows)
beschreiben die Installation der Windows-Werkzeuge. Die Node-Voraussetzung
entspricht dem hier verwendeten [Vite](https://vite.dev/guide/).

Im Projektverzeichnis in PowerShell:

```powershell
npm install
powershell -File .\scripts\setup-mpv.ps1
powershell -File .\scripts\setup-ffmpeg.ps1
npm exec -- tauri dev
```

Im bereits eingerichteten Projekt genügt `npm run desktop`. Der Befehl verwendet
eine vorhandene lokale Toolchain automatisch. `npm run desktop:build` erstellt
das Paket für die aktuelle Plattform.

Der erste Rust-Build lädt seine Cargo-Abhängigkeiten. Tauri startet den
Vite-Entwicklungsserver automatisch. `npm run dev` allein zeigt die Oberfläche
im Browser; echte Audiowiedergabe und Dateizugriffe benötigen das Desktopfenster.

Falls eine projektlokale Rust-/MSVC-Toolchain unter `.tools` eingerichtet wurde,
kann sie über den Helfer verwendet werden:

```powershell
powershell -File .\scripts\desktop.ps1 dev
```

Oder für mehrere Befehle in derselben PowerShell-Sitzung:

```powershell
. .\scripts\toolchain.ps1
npm exec -- tauri dev
```

Die Helfer erkennen vorhandene lokale Werkzeuge; sie installieren keine
Toolchain. `.tools` wird nicht mit dem Quellcode verteilt. Ohne lokale Werkzeuge
verwenden sie die regulär installierte Toolchain.

## libmpv-Runtime

`scripts/setup-mpv.ps1` lädt den gepinnten Windows-x64-Build
`mpv-dev-x86_64-20261111-git-bcde6b93cc.7z` von shinchiro, prüft Archiv und DLL
anhand festgehaltener SHA256-Werte und legt `libmpv-2.dll` unter
`src-tauri/resources/mpv/` ab. Dieser Anbieter wird auf der
[mpv-Installationsseite](https://mpv.io/installation/) genannt. Das Skript benötigt
Windows `tar.exe`; ein libmpv-SDK oder eine systemweite mpv-Installation ist zum
Kompilieren nicht erforderlich.

Die DLL wird als App-Ressource paketiert. Buildherkunft und Prüfsummen stehen in
`src-tauri/resources/mpv/build-manifest.json`; Lizenzhinweise in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Eine eigene kompatible Runtime kann über `AUDIOPLAYER_MPV_PATH` angegeben werden,
entweder als Bibliotheksdatei oder als Ordner. Die gebündelte Runtime hat Vorrang.
Auf macOS/Linux sind eine passende `libmpv.2.dylib` beziehungsweise `libmpv.so.2`
und deren native Abhängigkeiten selbst bereitzustellen.

Analyse, Wellenform, Editorvorschau und Export verwenden zusätzlich ein separat
gebündeltes FFmpeg-Programm. `scripts/setup-ffmpeg.ps1` prüft Archiv und Programm
gegen feste SHA256-Werte und installiert die Windows-x64-Runtime unter
`src-tauri/resources/ffmpeg/`. Auf anderen Plattformen wird eine passende native
FFmpeg-Runtime benötigt; die Paketierung ist dort noch nicht geprüft.

## Prüfen und Windows-Paket bauen

```powershell
. .\scripts\toolchain.ps1
npm run build
cargo test
cargo check -p sonora
npm run test:ui
npm run test:locales
```

Die nativen Wiedergabetests sind standardmäßig ignoriert, weil sie eine
libmpv-Runtime benötigen. Nach deren Einrichtung lassen sie sich explizit mit
Null-Audioausgabe, ohne Ausgabe an die Lautsprecher, ausführen:

```powershell
cargo test -p player-infrastructure native_engine -- --ignored --nocapture
cargo test -p player-application --test native_playback -- --ignored --nocapture
```

Mit beiden eingerichteten Runtimes führt
`cargo test --workspace --offline -- --include-ignored` zusätzlich sämtliche
FFmpeg-Analyse-, Metadaten- und Exporttests aus.

Ein Windows-x64-NSIS-Installationspaket entsteht mit:

```powershell
powershell -File .\scripts\desktop.ps1 build --bundles nsis
```

Mit regulär installierter Toolchain ist der entsprechende Befehl
`npm exec -- tauri build --bundles nsis`. Tauri gibt den fertigen Installerpfad
am Ende des Builds aus; normalerweise liegt er unter `target/release/bundle/nsis/`.
Zum erstmaligen Erstellen werden gegebenenfalls die NSIS-Buildwerkzeuge geladen.

## Eigenes Testprofil

Standardmäßig verwendet Song Player die plattformspezifischen Tauri-Verzeichnisse für
App-Daten und Covercache. Die Datenbank heißt `sonora.sqlite`. Für einen Test mit
getrennten Einstellungen und Bibliothek zuerst laufende Song-Player-Fenster schließen:

```powershell
$env:SONORA_DATA_DIR = Join-Path (Get-Location) '.tmp\test-profile'
powershell -File .\scripts\desktop.ps1 dev
```

Das Testprofil legt Datenbank und `covers/` im angegebenen Ordner an. Die Variable
gilt für die aktuelle Shell und ihre Kindprozesse. Zum normalen Profil zurück:

```powershell
Remove-Item Env:SONORA_DATA_DIR
```

## Stand und Grenzen

Windows ist die erste Zielplattform. macOS und Linux sind im Aufbau
berücksichtigt, wurden aber noch nicht zur Laufzeit geprüft; ihre Paketierung
benötigt passende native Runtimes. Die Systemmediensteuerung ist aktuell nur
unter Windows angebunden.

Crossfade, allgemeine Tag-Bearbeitung, A–B-Schleifen und gewichtete Mischstrategien
sind noch nicht implementiert. Eine bitgenaue Audioausgabe wird nicht garantiert.
Formatunterstützung, Gapless und Geräteverhalten hängen auch vom
konkreten Engine-Build und Audiogerät ab.

Der sichtbare Name lautet jetzt Song Player. Die internen Kennungen
`dev.sonora.player`, `sonora.sqlite`, `Sonora.Audio`, `SONORA_DATA_DIR` sowie die
Exportformate `sonora-theme` und `sonora-settings` bleiben für vorhandene Profile,
Dateizuordnungen und Sicherungen erhalten. Einstellungsdateien enthalten weder
Musikdateien noch Playlistbilder, Favoriten oder Hörverlauf; der Verlauf hat
einen eigenen Export. Automatisches Wiederfinden verschobener Titel verwendet
nur eindeutige Inhaltsprüfsummen fehlender Dateien, keine geratenen Metadaten.

Details zu Schichtengrenzen, Tests und Sprachressourcen enthält
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).
