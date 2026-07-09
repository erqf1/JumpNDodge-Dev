import os
import sys
import json
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
# ============================================


def log(msg):
    print(f"[UPDATER] {msg}", flush=True)


def check_internet(timeout=5):
    try:
        urllib.request.urlopen("https://api.github.com", timeout=timeout)
        return True
    except Exception:
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
    with urllib.request.urlopen(req, timeout=10) as resp:
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
    with urllib.request.urlopen(req, timeout=120) as resp, open(dest, "wb") as f:
        shutil.copyfileobj(resp, f)


def merge_copy(src_dir, dst_dir, rel_path=""):
    """Kopiert src_dir nach dst_dir, überschreibt nur was in src_dir existiert -
    Dateien, die nur in dst_dir liegen (z.B. eigene Save-Daten), bleiben unangetastet.
    In PROTECTED_DIRS gelisteten Ordnern werden bestehende Dateien nie überschrieben,
    auch wenn das Update eine neue Version davon mitbringt - nur wirklich neue Dateien
    (die dort noch nicht existieren) werden ergänzt, damit z.B. eigene Sound-Anpassungen
    nicht durch Standard-Dateien aus dem Update ersetzt werden."""
    os.makedirs(dst_dir, exist_ok=True)
    protected = rel_path in PROTECTED_DIRS
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
            shutil.copy2(s, d)


def apply_update(extracted_root, app_dir):
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
            shutil.copy2(src, dst)
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


def main():
    app_dir = os.path.dirname(os.path.abspath(sys.argv[0]))
    os.chdir(app_dir)
    log(f"Arbeitsverzeichnis: {app_dir}")

    log("Prüfe Internetverbindung...")
    if not check_internet():
        log("Kein Internet gefunden -> starte ohne Update-Check")
        launch_app(app_dir)
        return

    local_version = read_local_version(os.path.join(app_dir, VERSION_FILE))
    log(f"Lokale Version: {local_version}")
    local_parsed = parse_version(local_version) or (0, 0, 0)

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

            apply_update(extract_dir, app_dir)

            with open(os.path.join(app_dir, VERSION_FILE), "w", encoding="utf-8") as f:
                f.write(remote_tag.lstrip("v"))
            log("Update abgeschlossen")
    except Exception as e:
        log(f"Update fehlgeschlagen, starte trotzdem mit alter Version: {e}")

    launch_app(app_dir)


if __name__ == "__main__":
    main()
