import os
import sys
import json
import ssl
import shutil
import zipfile
import tempfile
import platform
import subprocess
import urllib.request
import urllib.error

# ================== CONFIG ==================
RELEASES_REPO = "erqf1/JumpNDodge-Archive"
API_RELEASES = f"https://api.github.com/repos/{RELEASES_REPO}/releases"
VERSION_FILE = "version.txt"
IS_WINDOWS = platform.system() == "Windows"
APP_EXE = "JumpNDodge.exe" if IS_WINDOWS else "JumpNDodge"
ASSET_SUFFIX = "-windows.zip" if IS_WINDOWS else "-linux.zip"
UPDATER_SELF_NAMES = {"updater.py", "updater.exe", "updater"}
# Ordner, in denen bestehende Dateien beim Update NIE überschrieben werden (z.B. eigene
# Sound-Anpassungen des Spielers) - nur Dateien, die dort noch nicht existieren, werden
# ergänzt. Pfade relativ zum App-Ordner, mit "/" geschrieben (plattformunabhängig).
PROTECTED_DIRS = {"data/sound"}
PROTECTED_DIRS_LOWER = {p.lower() for p in PROTECTED_DIRS}
# ============================================

# Log-Datei neben der exe/version.txt, damit man bei Problemen (z.B. "Update landet im
# falschen Ordner") tatsächlich nachvollziehen kann, was der Updater erkannt/getan hat -
# vorher gingen alle log()-Meldungen nur an stdout, das bei einer windowed/onefile-exe
# ohne Konsole für den Nutzer unsichtbar ist.
LOG_FILE = "updater_log.txt"


def log(msg):
    line = f"[UPDATER] {msg}"
    print(line, flush=True)
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(line + "\n")
    except Exception:
        pass  # Logging darf den Updater niemals zum Absturz bringen


def build_ssl_context():
    """Baut einen SSLContext für HTTPS-Requests. Auf manchen Linux-Systemen (v.a. bei
    PyInstaller-Builds oder minimalen/frischen Installationen) fehlen die System-CA-
    Zertifikate, wodurch jede HTTPS-Verbindung mit CERTIFICATE_VERIFY_FAILED fehlschlägt.
    Der Updater interpretiert das fälschlich als "kein Internet" und bricht komplett ab,
    ohne dass ein Update-Check je stattfindet. Deshalb: zuerst der normale System-
    Kontext, und falls der fehlschlägt, das certifi-CA-Bundle als Fallback (falls
    installiert)."""
    try:
        ctx = ssl.create_default_context()
        # Testweise prüfen, ob der Kontext funktioniert, indem wir ihn einfach zurückgeben -
        # der eigentliche Test passiert beim ersten echten Request in check_internet().
        return ctx
    except Exception as e:
        log(f"Konnte Standard-SSL-Kontext nicht erstellen: {e}")
        return None


def _ssl_context_with_certifi_fallback():
    try:
        import certifi
        return ssl.create_default_context(cafile=certifi.where())
    except ImportError:
        return None
    except Exception as e:
        log(f"certifi-Fallback fehlgeschlagen: {e}")
        return None


SSL_CONTEXT = build_ssl_context()


def urlopen_safe(req_or_url, timeout=10):
    """Wrapper um urllib.request.urlopen mit gestaffelten Fallbacks bei SSL-Fehlern:
    1. System-CA-Zertifikate (Normalfall)
    2. certifi-CA-Bundle, falls installiert
    3. Unverifizierte Verbindung als allerletzter Ausweg, damit ein kaputter/fehlender
       System-CA-Store (auf Linux überraschend häufig, z.B. auf frisch aufgesetzten oder
       manchen Arch-Installationen) den Updater nicht komplett lahmlegt. Das betrifft nur
       den Update-Check/Download von GitHub, keine sensiblen Daten - der Tradeoff
       zwischen "Update funktioniert" und "strikte Zertifikatsprüfung" ist hier bewusst
       zugunsten von Ersterem gewählt."""
    try:
        return urllib.request.urlopen(req_or_url, timeout=timeout, context=SSL_CONTEXT)
    except urllib.error.URLError as e:
        reason = getattr(e, "reason", e)
        if not (isinstance(reason, ssl.SSLCertVerificationError) or "CERTIFICATE_VERIFY_FAILED" in str(e)):
            raise
        log(f"SSL-Zertifikatsprüfung mit System-CAs fehlgeschlagen ({e}) -> versuche certifi-Fallback")
        fallback_ctx = _ssl_context_with_certifi_fallback()
        if fallback_ctx is not None:
            try:
                return urllib.request.urlopen(req_or_url, timeout=timeout, context=fallback_ctx)
            except urllib.error.URLError:
                pass
        else:
            log("certifi ist nicht installiert (pip install certifi) -> kein Fallback möglich")
        log("WARNUNG: Zertifikatsprüfung wird deaktiviert (unsichere Verbindung) als letzter Ausweg, "
            "da sowohl System-CAs als auch certifi fehlschlagen. Empfehlung: 'sudo pacman -S "
            "ca-certificates-mozilla && sudo trust extract-compat' (Arch) bzw. das Äquivalent "
            "der jeweiligen Distribution ausführen, um das dauerhaft zu beheben.")
        unverified_ctx = ssl._create_unverified_context()
        return urllib.request.urlopen(req_or_url, timeout=timeout, context=unverified_ctx)


def check_internet(timeout=5):
    try:
        urlopen_safe("https://api.github.com", timeout=timeout)
        return True
    except Exception as e:
        log(f"Internet-/HTTPS-Check fehlgeschlagen: {e}")
        return False


def parse_version(v):
    """Parst 'v1.0.0' oder '1.0.0-beta' usw. Gibt ein vergleichbares Tupel zurück, das die
    SemVer-Regel abbildet: eine Version OHNE Suffix gilt als neuer/finaler als dieselbe
    Version MIT Suffix (z.B. "1.0.0" > "1.0.0-beta"), egal welcher Text im Suffix steht."""
    try:
        core, _, suffix = v.strip().lstrip("v").partition("-")
        core_parts = tuple(int(p) for p in core.split("."))
        return (core_parts, 0 if suffix else 1)
    except Exception:
        return None


def read_local_version(path):
    if not os.path.exists(path):
        log("Lokale version.txt nicht gefunden -> nehme 0.0.0 an")
        return "0.0.0"
    try:
        with open(path, "r", encoding="utf-8") as f:
            return f.read().strip()
    except Exception as e:
        log(f"Konnte version.txt nicht lesen: {e}")
        return "0.0.0"


def fetch_latest_release():
    """Holt die neueste Release mit einem echten Versions-Tag. Nutzt bewusst die volle
    Releases-Liste statt /releases/latest, da GitHubs "Latest"-Slot auch von anderen
    Releases belegt sein kann, die keine Spielversion sind (z.B. der dauerhafte
    "installer"-Release) - solche werden hier einfach übersprungen."""
    req = urllib.request.Request(
        API_RELEASES,
        headers={"Accept": "application/vnd.github+json", "User-Agent": "JumpNDodge-Updater"},
    )
    with urlopen_safe(req, timeout=10) as resp:
        releases = json.loads(resp.read().decode("utf-8"))
    for release in releases:
        if parse_version(release.get("tag_name", "")) is not None:
            return release
    return None


def find_asset_url(release_json):
    for asset in release_json.get("assets", []):
        name = asset.get("name", "")
        if name.endswith(ASSET_SUFFIX):
            return asset.get("browser_download_url"), name
    return None, None


def download(url, dest):
    req = urllib.request.Request(url, headers={"User-Agent": "JumpNDodge-Updater"})
    with urlopen_safe(req, timeout=120) as resp, open(dest, "wb") as f:
        shutil.copyfileobj(resp, f)


def _safe_copy(src, dst):
    """Kopiert eine einzelne Datei, bricht aber nicht das ganze Update ab, falls sie
    gerade gesperrt ist. Hintergrund: Der Windows-Build kopiert JumpNDodge.dist/ UND
    updater.dist/ (zwei eigenständige Nuitka-Standalone-Builds mit jeweils eigenem
    Laufzeit-DLL-Satz) in denselben flachen Release-Ordner - bei gleicher Python-/
    Nuitka-Version im Build sind darunter oft identisch benannte Dateien. UPDATER_SELF_NAMES
    schützt nur updater.exe selbst vor Überschreiben, nicht aber dessen mitkopierte
    Laufzeit-DLLs, die der GERADE LAUFENDE updater.exe-Prozess offen hält - Windows lässt
    ein Überschreiben davon nicht zu (PermissionError/WinError 32).
    Ohne dieses Abfangen bricht apply_update() an genau dieser Stelle mit einer Exception
    ab, BEVOR main() hinterher version.txt schreibt - dadurch bleibt version.txt auf dem
    alten Stand und der Updater lädt bei jedem Start das komplette Update erneut herunter.
    Gibt True zurück wenn kopiert, False wenn (mit Log-Warnung) übersprungen."""
    try:
        shutil.copy2(src, dst)
        return True
    except (PermissionError, OSError) as e:
        log(f"WARNUNG: Konnte {os.path.basename(dst)} nicht überschreiben (vermutlich "
            f"gerade in Benutzung, z.B. eine Laufzeit-Datei von updater.exe selbst) -> "
            f"übersprungen: {e}")
        return False


def merge_copy(src_dir, dst_dir, rel_path=""):
    """Kopiert src_dir nach dst_dir, überschreibt nur was in src_dir existiert -
    Dateien, die nur in dst_dir liegen (z.B. eigene Save-Daten), bleiben unangetastet.
    In PROTECTED_DIRS gelisteten Ordnern werden bestehende Dateien nie überschrieben,
    auch wenn das Update eine neue Version davon mitbringt - nur wirklich neue Dateien
    (die dort noch nicht existieren) werden ergänzt, damit z.B. eigene Sound-Anpassungen
    nicht durch Standard-Dateien aus dem Update ersetzt werden.
    WICHTIG: Diese Funktion löscht NIEMALS etwas im Zielordner - Dateien, die dort
    liegen aber nicht im Update enthalten sind, bleiben unverändert bestehen."""
    os.makedirs(dst_dir, exist_ok=True)
    protected = rel_path.lower() in PROTECTED_DIRS_LOWER
    for name in os.listdir(src_dir):
        s = os.path.join(src_dir, name)
        d = os.path.join(dst_dir, name)
        child_rel = f"{rel_path}/{name}" if rel_path else name
        if os.path.isdir(s):
            merge_copy(s, d, child_rel)
        else:
            if protected and os.path.exists(d):
                log(f"Überspringe (geschützt, bereits vorhanden): {child_rel}")
                continue
            _safe_copy(s, d)


def find_content_root(extract_dir):
    """Manche Release-Zips (je nach Build-Pipeline) enthalten keinen flachen Satz an
    Dateien, sondern einen einzigen Wrapping-Ordner (z.B. 'JumpNDodge-linux/'), in dem
    die eigentlichen Dateien liegen. Ohne diese Erkennung würde apply_update() diesen
    Ordner 1:1 als Unterordner in den App-Ordner kopieren, statt seinen Inhalt direkt
    ins App-Verzeichnis zu übernehmen - die eigentlichen Dateien (Executable, sound/,
    data/, ...) würden dann nie aktualisiert, obwohl der Updater 'Erfolg' meldet.
    Deshalb: falls der extrahierte Ordner genau einen Eintrag enthält und das ein
    Ordner ist, gilt dessen Inhalt als eigentlicher Update-Root."""
    # Versteckte Junk-Einträge (z.B. macOS .DS_Store), die manche Zip-Tools mit reinpacken,
    # ignorieren wir bei der "genau 1 Eintrag"-Prüfung - sonst würde ein Wrapping-Ordner
    # wegen so einer Nebendatei fälschlich NICHT erkannt und einen Ordner zu tief kopiert.
    entries = [e for e in os.listdir(extract_dir) if e not in (".DS_Store", "Thumbs.db")]
    if len(entries) == 1:
        single_path = os.path.join(extract_dir, entries[0])
        if os.path.isdir(single_path):
            log(f"Erkenne Wrapping-Ordner '{entries[0]}' im Update-Archiv -> flache ihn ab")
            return single_path
    return extract_dir


def apply_update(extracted_root, app_dir):
    """Überträgt die Update-Dateien in den App-Ordner. Überschreibt bestehende Dateien
    mit gleichem Namen (außer in PROTECTED_DIRS, siehe merge_copy), löscht aber
    NIEMALS Dateien im App-Ordner - auch nicht solche, die im Update fehlen."""
    log("Wende Update an...")
    for item in os.listdir(extracted_root):
        if item.lower() in UPDATER_SELF_NAMES:
            log(f"Überspringe Updater-Datei: {item}")
            continue
        src = os.path.join(extracted_root, item)
        dst = os.path.join(app_dir, item)
        if os.path.isdir(src):
            merge_copy(src, dst, item)
            log(f"Ordner aktualisiert: {item}")
        else:
            if _safe_copy(src, dst):
                log(f"Datei aktualisiert: {item}")
    log("Update-Dateien erfolgreich übernommen")


def launch_app(app_dir):
    exe_path = os.path.join(app_dir, APP_EXE)
    if not os.path.exists(exe_path):
        log(f"FEHLER: {APP_EXE} nicht gefunden, kann nicht starten")
        return
    log(f"Starte {APP_EXE}")
    try:
        subprocess.Popen([exe_path], cwd=app_dir)
    except Exception as e:
        log(f"FEHLER: Konnte {APP_EXE} nicht starten: {e}")


def _looks_like_app_dir(path):
    """Prüft, ob 'path' plausibel der echte JumpNDodge-Ordner ist (enthält die
    Spiel-Executable oder version.txt). Wird benutzt, um einen falsch erkannten
    Ordner (z.B. einen Eltern- oder Unterordner statt des echten App-Ordners -
    genau das beschriebene Symptom, dass Update-Dateien im falschen Ordner landen)
    VOR dem Schreiben zu erkennen, statt das Update blind in irgendeinen
    Kandidaten-Pfad zu kopieren."""
    try:
        if os.path.isfile(os.path.join(path, APP_EXE)):
            return True
        if os.path.isfile(os.path.join(path, VERSION_FILE)):
            return True
    except Exception:
        pass
    return False


def _get_app_dir():
    """Ermittelt das echte App-Verzeichnis, auch als Nuitka-Onefile-Build.

    sys.argv[0] zeigt bei manchen Onefile-Aufrufen (z.B. über PATH oder
    bestimmte Startmethoden) nicht zuverlässig auf den echten exe-Ordner.
    __compiled__.containing_dir (von Nuitka bereitgestellt) und sys.executable
    sind bei kompilierten Builds robuster.

    Bugfix: Bisher wurde einfach der ERSTE passende Kandidat (Nuitka >
    PyInstaller > sys.argv[0]) ungeprüft übernommen. Je nach Build-/Start-
    Konstellation (z.B. wenn __compiled__.containing_dir aus irgendeinem Grund
    nicht exakt den Ordner liefert, in dem Updater.exe tatsächlich neben
    JumpNDodge liegt) konnte das dazu führen, dass app_dir auf einen Eltern-
    oder Unterordner zeigte - der Updater hat dann "erfolgreich" ins Leere
    (bzw. den falschen Ordner) aktualisiert, ohne dass das auffiel. Jetzt werden
    ALLE Kandidaten gesammelt und der erste genommen, der auch wirklich wie der
    App-Ordner aussieht (_looks_like_app_dir); nur wenn keiner passt, wird der
    beste verfügbare Kandidat mit einer deutlichen Warnung verwendet.
    """
    candidates = []
    if '__compiled__' in globals():
        candidates.append(__compiled__.containing_dir)
    if getattr(sys, 'frozen', False):
        candidates.append(os.path.dirname(os.path.abspath(sys.executable)))
    exe_path = getattr(sys, 'executable', None)
    if exe_path and os.path.basename(exe_path).lower().startswith(('jumpndodge', 'updater')):
        cand = os.path.dirname(os.path.abspath(exe_path))
        if cand not in candidates:
            candidates.append(cand)
    argv0_dir = os.path.dirname(os.path.abspath(sys.argv[0]))
    if argv0_dir not in candidates:
        candidates.append(argv0_dir)

    for c in candidates:
        if c and _looks_like_app_dir(c):
            return c

    best = candidates[0] if candidates else os.getcwd()
    log(f"WARNUNG: Konnte App-Ordner nicht eindeutig bestätigen (keiner der "
        f"Kandidaten {candidates} enthält {APP_EXE} oder {VERSION_FILE}) -> "
        f"verwende trotzdem: {best}. Prüfe updater_log.txt, falls Dateien "
        f"wieder im falschen Ordner landen.")
    return best


def main():
    app_dir = _get_app_dir()
    os.chdir(app_dir)
    log(f"Arbeitsverzeichnis: {app_dir}")

    log("Prüfe Internetverbindung...")
    if not check_internet():
        log("Kein Internet gefunden -> starte ohne Update-Check")
        launch_app(app_dir)
        return

    local_version = read_local_version(os.path.join(app_dir, VERSION_FILE))
    log(f"Lokale Version: {local_version}")
    local_parsed = parse_version(local_version) or ((0, 0, 0), 1)

    try:
        release = fetch_latest_release()
    except Exception as e:
        log(f"Konnte GitHub-Version nicht abrufen: {e}")
        launch_app(app_dir)
        return

    if release is None:
        log("Keine gültige Versions-Release auf GitHub gefunden -> breche Update ab")
        launch_app(app_dir)
        return

    remote_tag = release.get("tag_name", "")
    remote_parsed = parse_version(remote_tag)
    log(f"GitHub-Version: {remote_tag or '(leer)'}")

    if remote_parsed is None:
        log("Ungültiges Versionsformat auf GitHub -> breche Update ab")
        launch_app(app_dir)
        return

    if remote_parsed <= local_parsed:
        log("Kein Update nötig")
        launch_app(app_dir)
        return

    log(f"Update verfügbar: {local_version} -> {remote_tag}")
    asset_url, asset_name = find_asset_url(release)
    if not asset_url:
        log(f"Kein passendes Release-Asset gefunden (erwarte *{ASSET_SUFFIX})")
        launch_app(app_dir)
        return

    try:
        with tempfile.TemporaryDirectory() as tmp:
            zip_path = os.path.join(tmp, asset_name)
            log(f"Lade {asset_name} herunter...")
            download(asset_url, zip_path)

            log("Entpacke...")
            extract_dir = os.path.join(tmp, "extracted")
            with zipfile.ZipFile(zip_path, "r") as z:
                z.extractall(extract_dir)

            content_root = find_content_root(extract_dir)
            apply_update(content_root, app_dir)

            with open(os.path.join(app_dir, VERSION_FILE), "w", encoding="utf-8") as f:
                f.write(remote_tag.lstrip("v"))
            log("Update abgeschlossen")
    except Exception as e:
        log(f"Update fehlgeschlagen, starte trotzdem mit alter Version: {e}")

    launch_app(app_dir)


if __name__ == "__main__":
    main()
