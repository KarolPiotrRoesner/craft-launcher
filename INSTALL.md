# ArtCraft Launcher unter Windows selbst bauen

Diese Anleitung setzt **keine Vorkenntnisse** voraus. Am Ende hast du eine `artcraft-launcher.exe`,
die du per Doppelklick startest. Rechne beim ersten Mal mit 30–60 Minuten, das meiste davon
Download- und Wartezeit.

Getestet für Windows 10 und 11 auf einem normalen 64-Bit-PC (Intel oder AMD).

> **Du willst den Launcher nur benutzen, nicht verändern?** Dann brauchst du nichts davon: lade
> die fertige `…-windows-x64.msi` von der
> [Release-Seite](https://github.com/storytold/craft-launcher/releases/latest) und installiere sie.

---

## Schritt 1: Die drei Werkzeuge installieren

Du brauchst **Git** (lädt den Quellcode), die **Microsoft C++ Build Tools** (der Linker, den Rust
unter Windows benutzt) und **Rust** selbst. Alle drei sind kostenlos.

Am einfachsten geht das mit `winget`, das in Windows 10/11 schon eingebaut ist. Öffne
**PowerShell** (Startmenü → „PowerShell“ eintippen → Enter) und füge die Befehle einzeln ein.

### 1a. Git

```powershell
winget install --id Git.Git -e
```

### 1b. Microsoft C++ Build Tools

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Das lädt einige Gigabyte und dauert eine Weile; ein Fenster des Visual-Studio-Installers zeigt den
Fortschritt. **Warte, bis es fertig ist.**

<details>
<summary>Ohne winget (von Hand)</summary>

1. <https://visualstudio.microsoft.com/de/visual-cpp-build-tools/> öffnen, „Build Tools
   herunterladen“ klicken und die Datei starten.
2. Im Installer den Haken bei **„Desktopentwicklung mit C++“** setzen. Rechts müssen
   „MSVC … C++-x64/x86-Buildtools“ und ein „Windows 11 SDK“ (oder Windows 10 SDK) angehakt sein,
   das ist die Voreinstellung.
3. **Installieren** klicken und warten.

Hast du schon Visual Studio 2022 (Community, Professional …) mit „Desktopentwicklung mit C++“,
kannst du diesen Schritt überspringen.
</details>

### 1c. Rust

```powershell
winget install --id Rustlang.Rustup -e
```

<details>
<summary>Ohne winget (von Hand)</summary>

Auf <https://rustup.rs> die Datei `rustup-init.exe` (64-bit) laden, starten und bei der Frage
einfach **Enter** drücken (Option 1, Standardinstallation).
</details>

### 1d. PowerShell neu öffnen und prüfen

**Schließe das PowerShell-Fenster und öffne ein neues**, damit Windows die neuen Programme findet.
Dann:

```powershell
git --version
rustc --version
cargo --version
```

Alle drei sollen eine Versionsnummer ausgeben. `rustc` muss **1.90 oder neuer** sein; ist es älter:

```powershell
rustup update stable
```

---

## Schritt 2: Den Quellcode holen

Lade das Projekt in deinen Benutzerordner (oder einen anderen Ordner deiner Wahl) herunter:

```powershell
cd $HOME
git clone https://github.com/KarolPiotrRoesner/craft-launcher
cd craft-launcher
```

(Das ist dieser Fork. Das Original liegt unter `https://github.com/storytold/craft-launcher`.)

---

## Schritt 3: Bauen und starten

Im Ordner `craft-launcher`:

```powershell
cargo run --release -p artcraft-launcher
```

Beim **ersten Mal** lädt Cargo alle Bibliotheken herunter und übersetzt sie. Das dauert je nach
Rechner 5–20 Minuten, und es sieht zwischendurch so aus, als würde nichts passieren. Das ist normal.
Danach öffnet sich das Fenster des ArtCraft Launchers.

Spätere Starts gehen in Sekunden, weil nur noch Geändertes neu übersetzt wird.

### Wo liegt die fertige .exe?

```text
craft-launcher\target\release\artcraft-launcher.exe
```

Die kannst du ab jetzt direkt per Doppelklick starten, ohne PowerShell.

---

## Schritt 4 (optional): Eine portable Version zum Weitergeben

Die gebaute `.exe` braucht auf einem anderen PC eventuell die „Visual C++ Redistributable“. Baue
sie deshalb zum Weitergeben so, dass sie alles mitbringt:

```powershell
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release --locked -p artcraft-launcher --target x86_64-pc-windows-msvc
```

Ergebnis: `target\x86_64-pc-windows-msvc\release\artcraft-launcher.exe`.

Kopiere sie in einen eigenen Ordner und lege daneben eine leere Datei **`portable.txt`** an. Dann
speichert der Launcher seine Einstellungen im Ordner `ArtCraftLauncherData\` direkt neben der
`.exe` statt in `%LOCALAPPDATA%`. Den Ordner kannst du zippen und weitergeben.

## Schritt 5 (optional): Ein MSI-Installationsprogramm bauen

Nur nötig, wenn du ein richtiges Setup mit Startmenü-Eintrag willst. Du brauchst zusätzlich .NET
und das WiX-Toolset:

```powershell
winget install --id Microsoft.DotNet.SDK.8 -e
# PowerShell neu öffnen, dann:
dotnet tool install --global wix --version 5.0.2
wix extension add -g WixToolset.UI.wixext/5.0.2 WixToolset.Util.wixext/5.0.2
```

Und dann im Projektordner:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\package.ps1 -Arch x64
```

Ergebnis in `dist\release\`: `artcraft-launcher-<version>-windows-x64.msi` und
`…-windows-x64-portable.zip`. Die Warnung, dass nicht signiert wurde, ist bei eigenen Builds
normal.

---

## Quellcode der Craft Apps herunterladen

Der Launcher kann von jeder Craft App das GitHub-Repo (`github.com/storytold/<app>`) als lokale
Git-Kopie mit vollständiger Historie anlegen und aktuell halten. Dafür muss **Git installiert**
sein (Schritt 1a).

- **Alle Repos herunterladen** (oben in der Kopfzeile) klont jedes fehlende Repo und aktualisiert
  die vorhandenen. Wenn eine Prüfung neue Commits gefunden hat, heißt der Button
  **Repos aktualisieren (N)**.
- **Repos prüfen** holt nur den neuesten Stand von GitHub (`git fetch`), ohne Dateien zu ändern,
  und zeigt danach, welche Repos neue Commits haben. Klonen, Prüfen und Aktualisieren laufen für
  bis zu vier Repos gleichzeitig.
- **Auf jeder Karte** steht unter der Versionszeile die **CODE**-Zeile mit der lokalen Version,
  z. B. `v0.6.0`. Die Version kommt aus der `Cargo.toml` des Klons. Die Farbe zeigt den Stand:
  **grün** heißt aktuell, **orange** heißt, es gibt Neues (`v0.6.0 · 3 Updates`) oder etwas wurde
  nicht aktualisiert (`v0.6.0 · geändert`). Beim Drüberfahren mit der Maus erscheint alles: Branch,
  Commit, Datum und gegebenenfalls der Grund. Während der Launcher arbeitet, steht dort „Wartet…“,
  „Wird geklont…“ oder „Wird abgerufen…“. Es gibt keine Meldungen unten rechts.
- Rechts in der CODE-Zeile:
  - das **Ordner-Symbol** öffnet den Repo-Ordner im Explorer (ist die App noch nicht geklont, den
    Repo-Ordner insgesamt),
  - das **Pfeil-Symbol** klont bzw. aktualisiert genau dieses eine Repo.
- In der **Tabellenansicht** steht der Stand unter dem Status. Dieselben Aktionen findest du im
  `…`-Menü und in der Detailansicht, die beim Klick auf eine Karte aufgeht.
- In einem schmalen Fenster sind die Aktionen unter **Repos** in der Kopfzeile zusammengefasst,
  zusammen mit **Repo-Ordner öffnen** und **Ordner ändern…**.

**Wo stelle ich den Pfad ein?** Unter **Einstellungen (⚙ oben rechts) → Quellcode →
Repo-Ordner**: „Durchsuchen…“ wählt einen Ordner, „Standard verwenden“ setzt ihn zurück, und
„Ordner öffnen“ zeigt ihn im Explorer. Standard ist `Dokumente\Craft Repos\<app>`.

Der Launcher überschreibt nie deine Arbeit. Er aktualisiert nur per Fast-Forward. Hat ein Klon
lokale Änderungen oder eigene Commits, wird er nur gefetcht, und du bekommst einen Hinweis. Ordner,
die kein Git-Repo sind oder auf ein anderes Repo zeigen, fasst er nicht an.

## Den Code aktualisieren

```powershell
cd $HOME\craft-launcher
git pull
cargo run --release -p artcraft-launcher
```

## Wenn etwas schiefgeht

| Meldung | Lösung |
|---|---|
| `git`, `cargo` oder `rustc` „wurde nicht erkannt“ | PowerShell schließen und neu öffnen. Im Terminal von RustRover oder VS Code das **ganze Programm** neu starten, ein neuer Tab reicht nicht. Oder im offenen Fenster: `$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')`. Hilft das alles nicht, Schritt 1 wiederholen |
| `linker 'link.exe' not found` | Die C++ Build Tools fehlen oder „Desktopentwicklung mit C++“ ist nicht angehakt (Schritt 1b) |
| `package … requires rustc 1.90` oder ähnlich | `rustup update stable` |
| Warnung `built without icon/version resources` | Harmlos: der Launcher läuft, nur das Icon der .exe fehlt. Abhilfe: Windows SDK im Visual-Studio-Installer nachinstallieren |
| `Zugriff verweigert` / Datei gesperrt beim Bauen | Den laufenden Launcher schließen (auch im Infobereich neben der Uhr) und erneut bauen. Manchmal hält auch der Virenscanner die Datei kurz fest |
| Bauen ist extrem langsam | Den Ordner `craft-launcher\target` im Windows-Defender als Ausnahme eintragen |
| Sonst etwas | Die Protokolldatei liegt unter `%LOCALAPPDATA%\ArtCraft Launcher\logs\artcraft-launcher.log` |

## Für Entwickler

Vor jedem Commit:

```powershell
rustup component add clippy rustfmt   # einmalig
cargo xtask ci                         # Formatierung, Clippy, Tests und der wasm-Check
```

Der wasm-Check braucht einmalig `rustup target add wasm32-unknown-unknown`. Mehr zum Aufbau des
Projekts steht in [`AGENTS.md`](AGENTS.md) und in der [`README.md`](README.md#build-from-source).
In RustRover reicht es, den Ordner `craft-launcher` zu öffnen. Oben rechts im Dropdown neben dem
grünen ▶ stehen dann zwei fertige Einträge (aus dem Ordner `.run/`):

- **Launcher bauen (exe)**: `cargo build --release -p artcraft-launcher`, Ergebnis in
  `target\release\artcraft-launcher.exe`
- **Launcher starten**: baut, falls nötig, und öffnet den Launcher
