display_surface = None

import pygame
import pygame.scrap  # Zwischenablage-Zugriff (Paste-Button bei Redeem-Key/Code)
import math
import colorsys  # Für die Regenbogenspur (HSV -> RGB Farbrotation)
import sys  # Systemfunktionen, z.B. für das Beenden des Programms
import random  # Zufallszahlen für verschiedene Spielmechaniken
import json  # JSON-Dateien für das Speichern und Laden von Daten
import time  # Zeitfunktionen für Delays und Timestamps
import os  # Betriebssystemfunktionen, z.B. für Pfadoperationen
import subprocess  # Linux-Zwischenablage-Fallback (xclip/xsel/wl-paste), siehe _get_clipboard_text()
from math import floor, sin, pi, exp  # Mathematische Funktionen
from datetime import timedelta, datetime  # Datum und Zeit für Daily Rewards
import ssl            # SSL-Kontext-Fallback für Online-Leaderboard-Requests (siehe _online_urlopen)
import urllib.request  # HTTP-Anfragen für Internetverbindung und Zeitabfrage
import urllib.error  # Fehlerbehandlung für HTTP-Anfragen
import urllib.parse  # URL-Encoding für Online-Leaderboard-Requests (Namen können Umlaute etc. enthalten)
import base64        # Base64-Kodierung für verschlüsselte Dateien
import hashlib       # SHA256-Prüfsummen für Dateiintegrität / Schlüsselableitung
import hmac          # HMAC für Verschlüsselung (Keystream) und Integritätsprüfung
import threading     # Dev-Konsole: liest stdin in einem Hintergrund-Thread; auch für Online-Requests
import queue         # Dev-Konsole: Thread-sichere Übergabe der eingelesenen Zeilen
import secrets       # Kryptographisch sichere Zufalls-Tokens fürs Online-Leaderboard
import unicodedata   # Namens-Normalisierung für den Online-Namensfilter

# Konsole auf UTF-8 umstellen - sonst erscheinen Umlaute in print()-Ausgaben (z.B. der
# Dev-Konsole) auf Windows als Mojibake, weil cmd.exe/PowerShell oft eine andere
# Standard-Codepage (cp1252/850) verwenden. sys.stdout/stdin sind bei der kompilierten
# --windows-console-mode=disable-exe None, deshalb der Guard.
try:
    if sys.stdout is not None:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if sys.stdin is not None:
        sys.stdin.reconfigure(encoding="utf-8")
except Exception:
    pass


# Basisverzeichnis korrekt setzen, egal ob PyInstaller-, Nuitka-Onefile-Build
# oder normales Python-Skript.
# - PyInstaller setzt sys.frozen -> Verzeichnis der exe-Datei verwenden
# - Nuitka setzt sys.frozen NICHT, stellt aber __compiled__.containing_dir
#   bereit; bei Onefile-Builds zeigt __file__ sonst auf einen internen
#   Temp-Entpackordner statt auf den echten Ordner neben der exe
# - sys.executable ist ein zusätzlicher Fallback für sonstige Freeze-Tools
if getattr(sys, 'frozen', False):
    BASE_DIR = os.path.dirname(sys.executable)  # Verzeichnis der exe-Datei (PyInstaller)
elif '__compiled__' in globals():
    BASE_DIR = __compiled__.containing_dir  # Verzeichnis der exe-Datei (Nuitka)
elif getattr(sys, 'executable', None) and os.path.basename(sys.executable).lower().startswith('jumpndodge'):
    BASE_DIR = os.path.dirname(os.path.abspath(sys.executable))
else:
    # Andernfalls verwende das Verzeichnis der Python-Datei
    BASE_DIR = os.path.dirname(os.path.abspath(__file__))

def resource_path(relative_path):
    """Gibt den absoluten Pfad für Ressourcen zurück, sowohl in .py als auch in .exe

    Diese Funktion stellt sicher, dass Ressourcen wie Sounds und Bilder
    korrekt gefunden werden, egal ob das Programm als Skript oder als
    kompilierte exe-Datei läuft.

    Args:
        relative_path (str): Relativer Pfad zur Ressource

    Returns:
        str: Absoluter Pfad zur Ressource oder relativer Pfad bei Fehler
    """
    try:
        return os.path.join(BASE_DIR, relative_path)
    except Exception as e:
        print(f"Fehler beim Bestimmen des Ressourcenpfads: {e}")
        return relative_path
    
# =============================
# Setup & Init
# =============================

# Initialisiere Pygame und seine Untermodule
pygame.init()  # Hauptinitialisierung von Pygame
pygame.mixer.init()  # Initialisierung des Audio-Systems für Musik und Sounds

# --- Hilfsfunktion: Physikalische Mauskoordinaten auf logische base_surface Koordinaten abbilden ---
def get_mouse_pos_logical():
    """Gibt die Mausposition abgebildet auf die logische Spieloberfläche (WIDTH x HEIGHT) zurück.

    Verwendet offset_x, offset_y, scale_x, scale_y, die in set_display_mode() berechnet werden.
    Diese Funktion ist wichtig für die korrekte Mausinteraktion im Vollbildmodus,
    wo die physische Bildschirmgröße von der logischen Spielgröße abweicht.

    Returns:
        tuple: (lx, ly) - Logische x,y Koordinaten
    """
    global _last_mouse_phys_pos, _kb_nav_active
    try:
        mx, my = pygame.mouse.get_pos()  # Physikalische Mausposition
        # Echte Mausbewegung schaltet den (nur per Tastendruck aktivierten) Fokus-Ring
        # wieder aus - man ist dann wieder "in der Maus", bis erneut eine Pfeiltaste kommt.
        if _last_mouse_phys_pos is not None and (mx, my) != _last_mouse_phys_pos:
            _kb_nav_active = False
        _last_mouse_phys_pos = (mx, my)
        lx = int((mx - offset_x) / scale_x)  # Umrechnung in logische x-Koordinate
        ly = int((my - offset_y) / scale_y)  # Umrechnung in logische y-Koordinate
        return (lx, ly)
    except Exception:
        # Fallback: Gibt die physikalische Position zurück, falls Berechnung fehlschlägt
        return pygame.mouse.get_pos()



# Liste für Admin-Power-Rechtecke (wird später gefüllt)
admin_power_rects = []

# Spielfeld-Abmessungen (logische Größe)
WIDTH, HEIGHT = 800, 600
_FULLSCREEN_BASE_W, _FULLSCREEN_BASE_H = 800, 600

# Erstelle das Hauptfenster mit der angegebenen Größe
# Hinweis: 'ül' scheint ein Tippfehler zu sein, wahrscheinlich sollte es 'ul' oder nichts sein
display_surface = pygame.display.set_mode((WIDTH, HEIGHT), pygame.RESIZABLE)
base_surface = pygame.Surface((WIDTH, HEIGHT))
pygame.display.set_caption("JumpNDodge")  # Fenstertitel setzen
_icon_surf = None
_custom_ico_path = None
try:
    import glob as _g
    _ico_candidates = _g.glob(os.path.join(BASE_DIR, "*.ico"))
    if _ico_candidates:
        _custom_ico_path = _ico_candidates[0]
        _icon_surf = pygame.image.load(_custom_ico_path).convert_alpha()
except Exception:
    _icon_surf = None
if _icon_surf is None:
    _icon_surf = pygame.Surface((32, 32))
    _icon_surf.fill((18, 12, 45))
    pygame.draw.rect(_icon_surf, (70, 110, 210), pygame.Rect(2, 2, 28, 28), border_radius=6)
    pygame.draw.rect(_icon_surf, (255, 210, 40), pygame.Rect(2, 2, 28, 28), width=2, border_radius=6)
    _icon_font = pygame.font.SysFont("arial", 13, bold=True)
    _icon_lbl = _icon_font.render("JD", True, (255, 215, 50))
    _icon_surf.blit(_icon_lbl, (16 - _icon_lbl.get_width() // 2, 16 - _icon_lbl.get_height() // 2))
pygame.display.set_icon(_icon_surf)

scale_x = 1.0
scale_y = 1.0
offset_x = 0
offset_y = 0

# Farbdefinitionen für das Spiel (RGB-Werte)
WHITE = (255, 255, 255)      # Weiß
BLACK = (0, 0, 0)            # Schwarz
GRAY = (220, 220, 220)       # Hellgrau
DARK_GRAY = (170, 170, 170)  # Dunkelgrau
RED = (255, 0, 0)            # Rot
BLUE = (0, 0, 255)           # Blau
GREEN = (0, 200, 0)          # Grün
GOLD = (255, 215, 0)         # Gold
PURPLE = (180, 80, 255)      # Lila
OVERLAY_BG = (20, 20, 30)    # Dunkler Hintergrund für Overlays

# Hintergrund-Sterne: [x(0-1), y(0-1), base_r, phase, drift_phase, drift_amp_x, drift_amp_y]
import random as _rng
_bg_stars = [
    [_rng.random(), _rng.random(), _rng.uniform(1.2, 3.0),
     _rng.uniform(0, 6.28), _rng.uniform(0, 6.28),
     _rng.uniform(0.008, 0.022), _rng.uniform(0.006, 0.018)]
    for _ in range(55)
]
# Hintergrund-Ringe: [rx(0-1), ry(0-1), base_rad, w, phase, drift_px]
_bg_rings = [
    [0.08, 0.18, 22, 2, 0.00, 0.012], [0.22, 0.55, 35, 2, 0.52, 0.009],
    [0.38, 0.12, 18, 2, 1.05, 0.015], [0.55, 0.72, 28, 2, 1.57, 0.011],
    [0.70, 0.30, 42, 2, 2.09, 0.008], [0.85, 0.60, 20, 2, 2.62, 0.013],
    [0.15, 0.80, 30, 2, 3.14, 0.010], [0.92, 0.15, 25, 2, 3.67, 0.014],
    [0.48, 0.90, 16, 2, 4.19, 0.016], [0.65, 0.08, 38, 2, 4.71, 0.009],
    [0.30, 0.42, 14, 2, 5.24, 0.017], [0.78, 0.85, 32, 2, 5.76, 0.011],
]
_gui_scale_dragging = False
_custom_speed_dragging = False
_custom_min_delay_dragging = False
_custom_max_delay_dragging = False
_ds_slider_edit = None  # Index (0/1/2) des per Tastatur "aktivierten" Custom-Sliders, sonst None

def draw_bg_effects(surface, w, h, show_rings=True):
    """Animierte Sterne (Herzschlag-Puls) und optional pulsierende/driftende Ringe."""
    import math
    animated   = settings.get("bg_animated", True)
    show_stars = settings.get("bg_stars", True)
    rings_on   = settings.get("bg_rings_enabled", True)
    t = (pygame.time.get_ticks() / 1000.0) if animated else 0.0
    dark = is_dark_mode()
    mono = settings.get("mono_bg", False)

    area_ratio = (w * h) / (800 * 600)
    max_stars = max(8, int(55 * area_ratio))

    if show_rings and rings_on:
        ring_cols_light = [(180,200,220),(150,180,210),(160,190,215),(170,205,225),
                           (145,175,205),(165,195,218),(155,185,212),(175,200,222),
                           (148,178,208),(162,192,216),(172,202,220),(158,188,213)]
        ring_cols_dark  = [(100,150,200),(150,100,200),(200,150,100),(90,140,195),
                           (140,95,195),(195,145,95),(105,155,205),(155,105,205),
                           (205,155,105),(95,145,198),(148,98,198),(198,148,98)]
        rcols = ring_cols_light if not dark else ring_cols_dark
        mono_rcol = (40, 40, 40) if not dark else (210, 210, 210)
        _ring_scale = min(w, h) / 600.0
        for i, (rx, ry, base_rad, lw, phase, damp) in enumerate(_bg_rings):
            pulse = math.sin(t * 1.1 + phase) * 6
            drift_x = math.sin(t * 0.5 + phase) * damp * 1.8
            drift_y = math.cos(t * 0.4 + phase) * damp * 1.8
            cx = int((rx + drift_x) * w)
            cy = int((ry + drift_y) * h)
            rad = max(4, int((base_rad + pulse) * _ring_scale))
            col = mono_rcol if mono else rcols[i % len(rcols)]
            pygame.draw.circle(surface, col, (cx, cy), rad, lw)
            if not dark and not mono:
                pygame.draw.circle(surface, tuple(max(0, c - 30) for c in col), (cx, cy), rad + 3, 1)

    if not show_stars:
        return

    if mono:
        star_cols = [(40, 40, 40)] if not dark else [(210, 210, 210)]
    else:
        star_cols_light = [(120, 160, 230), (220, 180, 60), (100, 200, 230), (230, 100, 140), (80, 180, 120)]
        star_cols_dark  = [(120, 160, 255), (255, 220, 80), (80, 200, 255), (255, 120, 180), (80, 220, 140)]
        star_cols = star_cols_light if not dark else star_cols_dark

    for i, (sx, sy, base_r, phase, dph, dax, day) in enumerate(_bg_stars[:max_stars]):
        beat = t * 2.2 + phase
        p1 = max(0.0, math.sin(beat)) ** 2
        p2 = max(0.0, math.sin(beat - 0.55)) ** 2
        pulse = p1 * 0.65 + p2 * 0.35
        r = max(1, base_r + pulse * 2.5)
        drift_x = math.sin(t * 0.28 + dph) * dax
        drift_y = math.cos(t * 0.22 + dph) * day
        cx = int((sx + drift_x) * w)
        cy = int((sy + drift_y) * h)
        col = star_cols[i % len(star_cols)]
        glow_r = int(r + (3 if not dark else 2))
        glow_col = tuple(min(255, c + (50 if not dark else 40)) for c in col)
        pygame.draw.circle(surface, glow_col, (cx, cy), max(2, glow_r), 1)
        pygame.draw.circle(surface, col, (cx, cy), max(1, int(r)))

def fill_bg(rings=False):
    """Füllt base_surface und zeichnet Hintergrund-Animationen.

    Ruft sync_window_size() auf, BEVOR base_surface gefüllt wird: Praktisch jeder
    Menü-Screen (Shop, Kosmetik-Shop, Loadout, Achievements, Settings, Admin, ...) hat
    seine EIGENE while-Schleife statt über die äußere Hauptschleife zu laufen, und ruft
    hier zu Beginn jedes Frames fill_bg() auf - das ist der einzige gemeinsame Punkt, den
    praktisch alle Menü-Screens durchlaufen. Vorher wurde sync_window_size() nur in der
    äußeren Hauptschleife und ein paar Gameplay-Loops aufgerufen; Screens mit eigener
    Schleife (z.B. der Shop) bekamen dadurch nie ein Update von WIDTH/HEIGHT/base_surface
    bei laufendem Fenster-Resize, wodurch _jnd_flip() das (falsch-große) base_surface beim
    finalen Blit nicht-proportional auf die neue Fenstergröße gestreckt hat."""
    sync_window_size()
    base_surface.fill(get_bg_color())
    draw_bg_effects(base_surface, WIDTH, HEIGHT, show_rings=rings)

# Sound-Variablen (werden später mit Sound-Objekten gefüllt)
sound_enabled = True          # Gibt an, ob Sounds aktiviert sind
menu_click_sound = None       # Sound für Menü-Klicks
click_sound = None            # Allgemeiner Klick-Sound
click_back_sound = None       # Sound für Zurück-Klicks
hit_sound = None              # Sound bei Kollision
gameover_sound = None         # Sound bei Game Over
heartbeat_sound = None        # Generierter Heartbeat-Sound für Almost Dead Effekt
oneup_sound = None            # Sound beim Einsammeln eines 1-UP-Powerups
menu_music_path = None        # Pfad zur Menü-Musik
ingame_music_path = None      # Pfad zur Spiel-Musik (aktuell gewählter Track)

# Auswählbare Ingame-Musik-Tracks: (settings-key, Anzeige-Name, Dateiname). Wird beim Laden
# automatisch aus allen data/sound/ingame_music_<name>.mp3 befüllt - der Dateiname IST der
# Anzeige-Name (siehe _discover_ingame_music_tracks()). Neue Tracks einfach als
# ingame_music_<name>.mp3 in den Sound-Ordner legen, kein Code-Change nötig.
INGAME_MUSIC_TRACKS = []
ingame_music_paths = {}       # track-key -> Pfad oder None (falls Datei fehlt)
_preview_channel = None       # pygame.mixer.Channel der aktuell laufenden Vorhör-Preview
_preview_track_key = None     # welcher Track gerade als Preview läuft (für Toggle/Icon)

# Dark Mode Farbpalette für dunkles Theme
DARK_BG = (30, 30, 40)          # Dunkler Hintergrund
DARK_PANEL = (50, 50, 60)       # Dunkle Panels
DARK_TEXT = (220, 220, 220)     # Heller Text für Dark Mode
DARK_BUTTON = (70, 70, 80)      # Dunkle Buttons
DARK_BUTTON_HOVER = (90, 90, 100)  # Hover-Effekt für Buttons im Dark Mode

# Schriftarten für verschiedene UI-Elemente
font = pygame.font.SysFont(None, 64)        # Große Schrift für Titel
menu_font = pygame.font.SysFont(None, 40)   # Mittlere Schrift für Menüs
small_font = pygame.font.SysFont(None, 26)  # Kleine Schrift für Details
mono_font = pygame.font.SysFont("consolas", 22)  # Monospace-Schrift für Code/Hints
tooltip_font = pygame.font.SysFont(None, 32)  # Schrift für Hover-Tooltips (besser lesbar als small_font)
tab_font = pygame.font.SysFont(None, 20)  # Kompakte Schrift für die Kosmetik-Tab-Leiste (6 Tabs müssen nebeneinander passen)

# Liste der unterstützten Sprachen im Spiel
languages = ["Deutsch", "English", "Русский", "Türkçe", "Schwiizerdütsch"]

def toggle_fullscreen():
    """Schaltet zwischen Vollbild- und Fenstermodus um.

    Aktualisiert die Einstellungen, speichert sie und wendet den neuen Modus an.
    Spielt einen Klick-Sound ab.
    """
    global settings
    settings["fullscreen"] = not settings.get("fullscreen", False)  # Umschalten des Vollbild-Flags
    save_section("settings", settings)  # Einstellungen speichern
    set_display_mode()  # Neuen Anzeigemodus anwenden
    play_sound(click_sound)  # Sound abspielen

def set_display_mode():
    """Setzt den Anzeigemodus basierend auf den Einstellungen.

    Im Vollbildmodus wird die logische Spielgröße (WIDTH x HEIGHT) auf die
    physische Bildschirmgröße skaliert. Im Fenstermodus wird 1:1 dargestellt.
    Berechnet Skalierungsfaktoren und Versätze für Mauskoordinaten.
    """
    global base_surface, display_surface, scale_x, scale_y, offset_x, offset_y, WIDTH, HEIGHT, _window_locked_for_gameplay
    if settings.get("fullscreen", False):
        info = pygame.display.Info()
        screen_w, screen_h = info.current_w, info.current_h
        display_surface = pygame.display.set_mode((screen_w, screen_h), pygame.FULLSCREEN)
        WIDTH = screen_w
        HEIGHT = screen_h
        base_surface = pygame.Surface((WIDTH, HEIGHT))
        scale_x = 1.0
        scale_y = 1.0
        offset_x = 0
        offset_y = 0
    else:
        display_surface = pygame.display.set_mode((WIDTH, HEIGHT), pygame.RESIZABLE)
        base_surface = pygame.Surface((WIDTH, HEIGHT))
        scale_x = 1.0
        scale_y = 1.0
        offset_x = 0
        offset_y = 0
        _window_locked_for_gameplay = False


    # Sicherheitsprüfungen: Stelle sicher, dass display_surface und base_surface existieren
    try:
        display_surface  # Versuche auf display_surface zuzugreifen
    except NameError:
        display_surface = pygame.display.get_surface()  # Fallback: Hole aktuelle Oberfläche
    try:
        base_surface  # Versuche auf base_surface zuzugreifen
    except NameError:
        base_surface = display_surface  # Fallback: Verwende display_surface

# =============================
# =============================
# Admin Settings
# =============================

# Admin-Passwort aus Umgebungsvariable oder Standardwert
ADMIN_PASSWORD = os.getenv("JND_ADMIN_PW", "letin123")

_ADMIN_KEY = "K9mX2pR7qN4wL1jH8tY5bV3cF6dG0sA2eU9iZ4xK7mP1rQ8nW5lJ3hT6yB0vC4fD2gS9aE7uI1oZ5xM8kP3rN6qW2jL0hT9yB4vC7fD1gS5aE3u"

_admin_keyfile_cache = {"result": False, "ts": 0.0}

def _check_admin_keyfile():
    """Prüft, ob eine Admin-Keyfile im Spielordner liegt.

    Das Scannen aller .txt-Dateien im Ordner (inkl. Disk-Reads) ist relativ teuer.
    Da diese Funktion pro Frame im "Mehr"-Screen aufgerufen wird (Event-Loop + Draw),
    wird das Ergebnis hier für 2 Sekunden gecacht, um das Laggen zu verhindern.
    """
    now = time.time()
    if now - _admin_keyfile_cache["ts"] < 2.0:
        return _admin_keyfile_cache["result"]

    import glob as _glob
    folder = BASE_DIR
    result = False
    for f in _glob.glob(os.path.join(folder, "*.txt")):
        try:
            content = open(f, encoding="utf-8", errors="ignore").read().strip()
            if content == _ADMIN_KEY:
                result = True
                break
        except Exception:
            pass

    _admin_keyfile_cache["result"] = result
    _admin_keyfile_cache["ts"] = now
    return result

# =============================
# Daily Rewards System
# =============================

def check_internet_connection():
    """Überprüft, ob eine Internetverbindung vorhanden ist.

    Versucht eine Verbindung zu Google herzustellen. Bei Erfolg wird True zurückgegeben,
    bei Fehlern (Timeout, keine Verbindung) False.

    Returns:
        bool: True wenn Internet verfügbar, False sonst
    """
    try:
        urllib.request.urlopen('http://www.google.com', timeout=5)  # Timeout nach 5 Sekunden
        return True
    except (urllib.error.URLError, urllib.error.HTTPError, OSError):
        return False

def get_current_datetime():
    """Holt das aktuelle Datum und Uhrzeit über das Internet.

    Verwendet die WorldTimeAPI für genaue Zeit. Bei Fehlern wird die lokale Systemzeit verwendet.

    Returns:
        str: ISO-Format Datum und Uhrzeit (YYYY-MM-DDTHH:MM:SS)
    """
    try:
        # Versuche das Datum von einem zuverlässigen Server zu holen
        with urllib.request.urlopen('http://worldtimeapi.org/api/timezone/Europe/Berlin', timeout=5) as response:
            data = json.loads(response.read().decode())
            return data['datetime']  # Vollständiges Datum und Uhrzeit im ISO-Format
    except:
        # Fallback: Lokales Datum verwenden, falls Internet nicht verfügbar
        return datetime.now().strftime('%Y-%m-%dT%H:%M:%S')

def get_current_date():
    """Holt das aktuelle Datum über das Internet.

    Schneidet die Zeit von get_current_datetime() ab und gibt nur das Datum zurück.

    Returns:
        str: Aktuelles Datum im Format YYYY-MM-DD
    """
    return get_current_datetime()[:10]  # Nur das Datum extrahieren (YYYY-MM-DD)

# =============================
# Online-Leaderboard (Client)
# =============================
# Alles hier ist best-effort: der Heimserver kann jederzeit offline sein
# (Stromausfall, Wartung, kein Port-Forwarding) - das Spiel muss davon
# unabhängig immer normal weiterlaufen. Deshalb: kurze Timeouts, jeder Fehler
# wird geschluckt und liefert None statt einer Exception, und alle Requests
# laufen in eigenen Threads, damit ein hängender Server nie einen Frame oder
# den Game-Over-Screen blockiert. Der Score selbst bleibt eine Client-Meldung
# (siehe server/app.py) - das Token verhindert nur Namens-Diebstahl.

ONLINE_ENABLED = True
ONLINE_SERVER_URL = "https://leaderboard.jumpndodge.download"


def _online_ssl_context():
    """Auf manchen Linux-Systemen (v.a. bei frischen/minimalen Installationen) fehlen
    die System-CA-Zertifikate, wodurch jede HTTPS-Verbindung mit CERTIFICATE_VERIFY_FAILED
    fehlschlägt - _online_request() fängt das als gewöhnlichen OSError ab und meldet
    einfach "kein Server", ohne dass der Spieler je erfährt, dass eigentlich nur die
    Zertifikatsprüfung das Problem ist. Gleiches Problem war schon im Updater (siehe
    updater.py build_ssl_context()/urlopen_safe(), v1.3.01) - hier dieselbe gestaffelte
    Lösung fürs Online-Leaderboard."""
    try:
        return ssl.create_default_context()
    except Exception:
        return None


_ONLINE_SSL_CONTEXT = _online_ssl_context()


def _online_ssl_context_certifi_fallback():
    try:
        import certifi
        return ssl.create_default_context(cafile=certifi.where())
    except Exception:
        return None


def _online_urlopen(req, timeout):
    """Wie urllib.request.urlopen(), aber mit gestaffelten Fallbacks bei SSL-Fehlern:
    System-CAs -> certifi-Bundle (falls installiert) -> unverifizierte Verbindung als
    letzter Ausweg, damit ein kaputter/fehlender System-CA-Store das Online-Leaderboard
    nicht komplett stummschaltet. Betrifft nur Score-Submits/Abfragen (nie sicherheits-
    kritisch, siehe Modulkommentar oben), der Tradeoff ist bewusst zugunsten von
    "funktioniert" gewählt - identisch zur Begründung im Updater."""
    try:
        return urllib.request.urlopen(req, timeout=timeout, context=_ONLINE_SSL_CONTEXT)
    except urllib.error.URLError as e:
        reason = getattr(e, "reason", e)
        if not (isinstance(reason, ssl.SSLCertVerificationError) or "CERTIFICATE_VERIFY_FAILED" in str(e)):
            raise
        fallback_ctx = _online_ssl_context_certifi_fallback()
        if fallback_ctx is not None:
            try:
                return urllib.request.urlopen(req, timeout=timeout, context=fallback_ctx)
            except urllib.error.URLError:
                pass
        return urllib.request.urlopen(req, timeout=timeout, context=ssl._create_unverified_context())


_DEFAULT_BANNED_WORDS = ["fuck", "shit", "bitch", "nigger", "faggot", "hurensohn", "wichser", "arschloch"]
_banned_words_cache = None
_ONLINE_LEET_MAP = str.maketrans({"0": "o", "1": "i", "3": "e", "4": "a", "5": "s", "7": "t", "@": "a", "$": "s"})


def _online_normalize_name(text):
    text = text.casefold()
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = text.translate(_ONLINE_LEET_MAP)
    return "".join(c for c in text if c.isalnum())


def _online_request(method, path, payload=None, timeout=5):
    """Führt einen HTTP-Request gegen ONLINE_SERVER_URL aus. Gibt bei jedem
    Fehler (kein Internet, Server offline, Timeout, ungültige Antwort) None
    zurück, statt eine Exception nach aussen durchzureichen."""
    if not ONLINE_ENABLED:
        return None
    url = ONLINE_SERVER_URL.rstrip("/") + path
    data = None
    # Eigener User-Agent statt urllib-Standard ("Python-urllib/3.x") - der wird von
    # Cloudflares Bot-Schutz vor dem Tunnel als Bot erkannt und geblockt (getestet:
    # ohne diesen Header 403 Forbidden, mit irgendeinem echten User-Agent 200 OK).
    headers = {"Accept": "application/json", "User-Agent": "JumpNDodge-Game-Client/1.0"}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with _online_urlopen(req, timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        try:
            return json.loads(e.read().decode("utf-8"))
        except Exception:
            return None
    except (urllib.error.URLError, OSError, ValueError):
        return None


def _online_request_async(method, path, payload=None, on_done=None):
    """Wie _online_request(), aber in einem Hintergrund-Thread - blockiert das
    Spiel nie, egal wie lange der Server braucht oder ob er gar nicht antwortet."""
    def _run():
        result = _online_request(method, path, payload)
        if on_done:
            try:
                on_done(result)
            except Exception:
                pass
    threading.Thread(target=_run, daemon=True).start()


def ensure_online_token():
    """Erzeugt beim ersten Online-Kontakt eines Profils einen geheimen,
    zufälligen Token und speichert ihn dauerhaft im Profil (wie stats/settings/
    etc., siehe save_current_profile()). Nur mit diesem Token darf später
    unter demselben Namen gepostet werden."""
    global online_token
    if not online_token:
        online_token = secrets.token_urlsafe(24)
        save_current_profile()
    return online_token


def get_cached_banned_words():
    """Gibt die gecachte Blockliste zurück (nie blockierend!). Startet beim
    allerersten Aufruf einen Hintergrund-Refresh von der echten Server-Liste
    und beantwortet diesen einen Aufruf sofort mit der kleinen eingebauten
    Fallback-Liste; alle folgenden Aufrufe sehen dann die echte Liste, sobald
    die Antwort da ist (oder bleiben beim Fallback, falls der Server nicht
    erreichbar ist)."""
    global _banned_words_cache
    if _banned_words_cache is None:
        _banned_words_cache = _DEFAULT_BANNED_WORDS

        def _on_done(result):
            global _banned_words_cache
            if result and result.get("ok") and isinstance(result.get("words"), list):
                _banned_words_cache = result["words"]

        _online_request_async("GET", "/api/banned_words", on_done=_on_done)
    return _banned_words_cache


def is_name_allowed(name):
    """Client-seitige Vorprüfung eines Profilnamens gegen die Blockliste - nur
    für sofortiges Feedback. Die verbindliche Prüfung passiert serverseitig in
    /api/submit (siehe server/filter.py)."""
    if not name or not name.strip():
        return False
    normalized = _online_normalize_name(name)
    if not normalized:
        return False
    for word in get_cached_banned_words():
        needle = _online_normalize_name(word)
        if needle and needle in normalized:
            return False
    return True


_submit_pending_local_name = None  # von submit_score_online()s on_done (Hintergrund-
# Thread) gesetzt, von sync_window_size() im Main-Thread angewendet (gleiches
# Race-Condition-Muster wie _rename_pending_local_name, siehe dort) - falls der
# Server bei der allerersten Registrierung einen kollidierenden Namen zu
# "Name#2" usw. aufgelöst hat (siehe server/app.py _resolve_unique_name()),
# muss das lokale Profil nachgeführt werden, sonst weicht der online geführte
# Name dauerhaft vom lokal angezeigten ab.


def submit_score_online(mode, difficulty, score):
    """Meldet einen Lauf best-effort an den Online-Leaderboard-Server. Nur für
    nicht-modded Standard-Läufe aufgerufen (siehe Aufrufstellen in run_game();
    Klassik wird bewusst nicht gemeldet)."""
    if not ONLINE_ENABLED or not current_profile_name:
        return
    token = ensure_online_token()
    sent_name = current_profile_name
    payload = {
        "name": sent_name,
        "token": token,
        "mode": mode,
        "difficulty": difficulty,
        "score": int(score),
        "powers_used": list(powers_state.get("equipped", [])),
        "profile_snapshot": {
            "stats": stats,
            "owned_powers": powers_state.get("owned", []),
            "equipped_powers": powers_state.get("equipped", []),
            "cosmetics": cosmetics_state,
            "player_color": settings.get("player_color", "blue"),
        },
    }

    def _on_done(result):
        global _submit_pending_local_name
        if result and result.get("ok"):
            confirmed_name = result.get("name")
            if confirmed_name and confirmed_name != sent_name:
                _submit_pending_local_name = confirmed_name

    _online_request_async("POST", "/api/submit", payload, on_done=_on_done)


def fetch_leaderboard(mode, difficulty, limit=50, names=None, on_done=None):
    """Holt die Rangliste für einen (Modus, Schwierigkeit)-Bucket asynchron.
    names (optional): nur diese Spielernamen zurückgeben (Freunde-Filter) -
    Rang wird dann relativ zu dieser Teilliste vergeben, nicht global."""
    path = f"/api/leaderboard?mode={urllib.parse.quote(mode)}&difficulty={urllib.parse.quote(difficulty)}&limit={limit}"
    if names:
        path += f"&names={urllib.parse.quote(','.join(names))}"
    _online_request_async("GET", path, on_done=on_done)


def fetch_my_rank(mode, difficulty, on_done=None):
    """Holt den eigenen Rang in einem (Modus, Schwierigkeit)-Bucket asynchron -
    auch ausserhalb der Top-N aus fetch_leaderboard(), fürs Rang-Dock."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "mode": mode, "difficulty": difficulty}
    _online_request_async("POST", "/api/my_rank", payload, on_done=on_done)


def search_players_online(query, on_done=None):
    """Sucht Spielernamen (Substring, serverseitig) asynchron."""
    path = f"/api/players/search?q={urllib.parse.quote(query)}"
    _online_request_async("GET", path, on_done=on_done)


def fetch_public_profile(name, on_done=None):
    """Holt das öffentliche Profil (Stats/Powers/Cosmetics/Bucket-Highscores)
    eines Spielers asynchron."""
    path = f"/api/players/{urllib.parse.quote(name)}"
    _online_request_async("GET", path, on_done=on_done)


def redeem_verified_key_online(key, on_done=None):
    """Löst einen Verified-Key für das aktuelle Profil ein. Ergebnis kommt
    asynchron über on_done(result) - result['ok'] + ggf. result['error']
    ('invalid_key', 'key_revoked', 'key_shared_revoked', 'unknown_player')."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "key": key.strip()}
    _online_request_async("POST", "/api/redeem_key", payload, on_done=on_done)


def redeem_code_online(code, on_done=None):
    """Löst einen allgemeinen Promo-Code (z.B. Gewinnspiel) für Coins ein -
    unabhängig vom Verified-Key-System. Ergebnis kommt asynchron über
    on_done(result); result['coins'] bei Erfolg, result['error'] sonst
    ('invalid_code', 'already_redeemed', 'unknown_player')."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "code": code.strip()}
    _online_request_async("POST", "/api/redeem_code", payload, on_done=on_done)


def rename_profile_online(new_name, on_done=None):
    """Ändert den Online-Anzeigenamen des aktuellen Profils (Server prüft
    Namensfilter + Kollision). Bei Erfolg wird auch das LOKALE Profil
    umbenannt (siehe on_done-Callback in der Aufrufstelle), damit lokaler und
    Online-Name nie auseinanderlaufen."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "new_name": new_name.strip()}
    _online_request_async("POST", "/api/rename", payload, on_done=on_done)


def send_friend_request_online(to_name, on_done=None):
    """Schickt eine Freundschaftsanfrage. Ergebnis über on_done(result);
    result['status'] ('pending'/'accepted') bei Erfolg, sonst result['error']
    ('already_friends', 'target_not_found', 'unknown_player')."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "to_name": to_name}
    _online_request_async("POST", "/api/friends/request", payload, on_done=on_done)


def respond_friend_request_online(from_name, accept, on_done=None):
    """Nimmt eine eingehende Anfrage an oder lehnt sie ab. Ablehnen räumt
    serverseitig in beide Richtungen auf, deckt also auch "eigene gesendete
    Anfrage zurückziehen" mit demselben Aufruf ab (siehe server/app.py
    friend_decline())."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    if accept:
        _online_request_async("POST", "/api/friends/accept", {"token": token, "from_name": from_name}, on_done=on_done)
    else:
        _online_request_async("POST", "/api/friends/decline", {"token": token, "other_name": from_name}, on_done=on_done)


def remove_friend_online(name, on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    _online_request_async("POST", "/api/friends/remove", {"token": token, "other_name": name}, on_done=on_done)


def fetch_friends_list(name, on_done=None):
    """Holt die bestätigte Freundesliste eines Spielers asynchron - öffentlich
    lesbar wie alles unter /api/players/<name>, kein Token nötig."""
    path = f"/api/friends/list?name={urllib.parse.quote(name)}"
    _online_request_async("GET", path, on_done=on_done)


def fetch_incoming_friend_requests(on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    _online_request_async("POST", "/api/friends/incoming", {"token": token}, on_done=on_done)


def create_clan_online(tag, name, on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "tag": tag.strip(), "name": name.strip()}
    _online_request_async("POST", "/api/clans/create", payload, on_done=on_done)


def join_clan_online(tag, on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    _online_request_async("POST", "/api/clans/join", {"token": token, "tag": tag.strip()}, on_done=on_done)


def leave_clan_online(on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    _online_request_async("POST", "/api/clans/leave", {"token": token}, on_done=on_done)


def kick_clan_member_online(target_name, on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    _online_request_async("POST", "/api/clans/kick", {"token": token, "target_name": target_name}, on_done=on_done)


def fetch_clan_info(tag, on_done=None):
    path = f"/api/clans/{urllib.parse.quote(tag)}"
    _online_request_async("GET", path, on_done=on_done)


def fetch_my_clan(on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    _online_request_async("POST", "/api/clans/mine", {"token": token}, on_done=on_done)


def search_clans_online(query, on_done=None):
    path = f"/api/clans/search?q={urllib.parse.quote(query)}"
    _online_request_async("GET", path, on_done=on_done)


def send_chat_message_online(conversation_type, target, body, on_done=None):
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "conversation_type": conversation_type, "target": target, "body": body}
    _online_request_async("POST", "/api/messages/send", payload, on_done=on_done)


def fetch_chat_messages(conversation_type, target, since_id=0, on_done=None):
    """POST statt GET, obwohl es inhaltlich ein Read ist - der Token wird hier
    zwingend gebraucht, um die Lese-Berechtigung serverseitig zu prüfen
    (Freundschaft/Clan-Mitgliedschaft), siehe Kommentar bei /api/my_rank in
    server/app.py für die gleiche Begründung (Token nie im Query-String)."""
    if not current_profile_name:
        return
    token = ensure_online_token()
    payload = {"token": token, "conversation_type": conversation_type, "target": target, "since_id": since_id}
    _online_request_async("POST", "/api/messages/list", payload, on_done=on_done)


_confirmed_online_deletions = []  # von _flush_pending_online_deletions()s on_done (Hintergrund-
# Thread) angehängt, von sync_window_size() im Main-Thread aus _pending_online_deletions
# entfernt und gespeichert - gleiches Race-Condition-Muster wie _rename_pending_local_name/
# _submit_pending_local_name (Dateizugriff darf nicht aus einem Hintergrund-Thread passieren).


def _flush_pending_online_deletions():
    """Versucht jeden noch offenen Online-Account-Löschauftrag erneut - Tokens,
    für die delete_profile() das lokale Profil schon entfernt hat, aber der
    Server-Kontakt (mangels Internet o.ä.) noch nicht bestätigt wurde. Wird
    sowohl direkt bei delete_profile() aufgerufen (Sofortversuch, falls online)
    als auch bei jedem Profil-Eintritt (siehe _enter_profile()), also praktisch
    bei jedem Programmstart, bis der Server irgendwann erfolgreich bestätigt."""
    pending = _load_profiles_file().get("_pending_online_deletions", [])
    for token in pending:
        def _on_done(result, token=token):
            if result and result.get("ok"):
                _confirmed_online_deletions.append(token)
        _online_request_async("POST", "/api/delete_account", {"token": token}, on_done=_on_done)


def _apply_confirmed_online_deletions():
    global _confirmed_online_deletions
    if not _confirmed_online_deletions:
        return
    pdata = _load_profiles_file()
    pending = pdata.get("_pending_online_deletions", [])
    for token in _confirmed_online_deletions:
        if token in pending:
            pending.remove(token)
    pdata["_pending_online_deletions"] = pending
    _save_profiles_file(pdata)
    _confirmed_online_deletions = []


def rename_current_profile_local(new_name):
    """Benennt das aktuell aktive LOKALE Profil um (verschiebt den Eintrag im
    _profiles-Dict auf den neuen Namen-Key). Wird nur nach erfolgreicher
    Server-Antwort von /api/rename aufgerufen, damit lokaler und Online-Name
    synchron bleiben."""
    global current_profile_name
    old_name = current_profile_name
    if not old_name or old_name == new_name:
        return
    pdata = _load_profiles_file()
    profiles = pdata.get("profiles", {})
    entry = profiles.pop(old_name, None)
    if entry is None:
        return
    entry["name"] = new_name
    profiles[new_name] = entry
    if pdata.get("active") == old_name:
        pdata["active"] = new_name
    if pdata.get("auto_start") == old_name:
        pdata["auto_start"] = new_name
    _save_profiles_file(pdata)
    current_profile_name = new_name


def check_local_verified_key_file():
    """Prüft beim Profil-Eintritt (siehe _enter_profile()), ob data/verified_key.txt
    existiert, und löst den darin stehenden Key automatisch im Hintergrund ein -
    der 99-Zeichen-Key muss so nicht von Hand eingetippt werden, einfach die
    Datei mit dem Key-Inhalt in den data/-Ordner legen."""
    key_path = os.path.join(_DATA_DIR, "verified_key.txt")
    try:
        if not os.path.isfile(key_path):
            return
        with open(key_path, encoding="utf-8") as f:
            key = f.read().strip()
    except OSError:
        return
    if key:
        redeem_verified_key_online(key)


# =============================
# Gecachte Zeit (Daily Reward & Sale)
# =============================
# Sowohl der Daily-Reward-Status als auch der tägliche Shop-Sale müssen wissen,
# "welcher Tag ist heute" - dafür wird bisher bei jedem Aufruf eine echte
# Internet-/Zeitanfrage gestellt. Diese Funktionen werden aber jeden Frame
# (60x pro Sekunde) aus der UI heraus aufgerufen, solange z.B. der Sale-Reiter
# oder das Daily-Reward-Menü offen ist -> das würde 60 Netzwerkanfragen pro
# Sekunde bedeuten. Stattdessen wird die echte Zeit hier nur alle
# _TIME_CACHE_INTERVAL Sekunden abgeglichen; dazwischen wird die zuletzt bekannte
# Zeit lokal weitergezählt (time.time()-Differenz), damit Countdown-Anzeigen
# trotzdem jede Sekunde flüssig weiterlaufen.
_TIME_CACHE_INTERVAL = 5.0  # Sekunden zwischen echten Zeit-Checks
_time_cache = {
    "checked_at": None,       # time.time() beim letzten echten Check
    "server_datetime": None,  # zuletzt ermittelte (Server- oder lokale) Uhrzeit
    "has_internet": True,
}

def _refresh_time_cache_if_needed():
    """Führt den echten (ggf. netzwerkbasierten) Zeit-Check höchstens alle
    _TIME_CACHE_INTERVAL Sekunden durch."""
    global _time_cache
    now_local = time.time()
    if _time_cache["checked_at"] is not None and now_local - _time_cache["checked_at"] < _TIME_CACHE_INTERVAL:
        return  # Letzter Check ist noch aktuell genug - kein neuer Netzwerkzugriff
    has_internet = check_internet_connection()
    if has_internet:
        try:
            server_dt = datetime.fromisoformat(get_current_datetime())
        except Exception:
            server_dt = _time_cache["server_datetime"] or datetime.now()
            has_internet = False
    else:
        server_dt = _time_cache["server_datetime"] or datetime.now()
    _time_cache["server_datetime"] = server_dt
    _time_cache["has_internet"] = has_internet
    _time_cache["checked_at"] = now_local

def get_cached_now():
    """Gibt (geschätzte_uhrzeit, hat_internet) zurück. Die Uhrzeit wird zwischen
    den echten Checks lokal weitergezählt, damit Countdowns weiterlaufen, ohne
    dass jeden Tick eine neue Zeitabfrage nötig ist."""
    _refresh_time_cache_if_needed()
    if _time_cache["server_datetime"] is None:
        return datetime.now(), _time_cache["has_internet"]
    elapsed = time.time() - _time_cache["checked_at"]
    return _time_cache["server_datetime"] + timedelta(seconds=elapsed), _time_cache["has_internet"]

def get_cached_current_date():
    """Wie get_current_date(), aber gecacht (siehe get_cached_now())."""
    now, has_internet = get_cached_now()
    return now.strftime("%Y-%m-%d"), has_internet

def calculate_daily_reward():
    """50-100 Basis-Coins + 90%-Kette Extra-Coins (stoppt bei 10%-Miss, max 150 Extra)."""
    base_coins = random.randint(50, 100)
    extra_coins = 0
    while extra_coins < 150 and random.random() < 0.90:
        extra_coins += 1

    total = base_coins + extra_coins
    if extra_coins >= 20:
        rarity = "Epic"
    elif extra_coins > 0:
        rarity = "Rare"
    else:
        rarity = "Normal"

    return total, {
        "base":   base_coins,
        "extra":  extra_coins,
        "total":  total,
        "rarity": rarity,
    }

def can_claim_daily_reward():
    """Überprüft, ob der Spieler heute bereits eine Belohnung erhalten hat.

    Nutzt den gecachten Zeit-Check (siehe get_cached_current_date), damit hier
    nicht bei jedem Aufruf (z.B. jeden Frame im Menü) eine echte Internet-/
    Zeitanfrage gestellt wird.

    Returns:
        tuple: (can_claim, message)
            - can_claim: Bool, ob Belohnung verfügbar
            - message: Grund oder aktuelles Datum
    """
    current_date, has_internet = get_cached_current_date()
    if not has_internet:
        return False, "Keine Internetverbindung"  # Internet erforderlich für Zeitabgleich

    last_claim_date = stats.get("last_daily_reward_date", "")

    if last_claim_date == current_date:
        return False, "Bereits heute erhalten"  # Schon heute beansprucht

    return True, current_date  # Verfügbar, mit aktuellem Datum

def claim_daily_reward():
    """Vergibt die tägliche Belohnung an den Spieler.

    Berechnet die Belohnung, aktualisiert die Stats und zeigt ein Overlay an.

    Returns:
        bool: True bei Erfolg, False wenn nicht verfügbar
    """
    can_claim, message = can_claim_daily_reward()

    if not can_claim:
        push_overlay(f"Daily Reward: {message}", (200, 100, 100), duration=2.0)
        return False

    # Belohnung berechnen und vergeben
    coins_earned, reward_stats = calculate_daily_reward()
    stats["coins"] = stats.get("coins", 0) + coins_earned
    stats["total_coins_earned"] = stats.get("total_coins_earned", 0) + coins_earned
    stats["last_daily_reward_date"] = message  # Aktuelles Datum speichern
    stats["last_daily_reward_stats"] = reward_stats  # Details für Anzeige speichern
    save_section("stats", stats)

    # Rarity-basierte Farbe für Overlay-Nachricht
    rarity_colors = {"Normal": (100, 100, 100), "Rare": (80, 150, 255), "Epic": (180, 80, 255)}
    color = rarity_colors.get(reward_stats["rarity"], GOLD)

    push_overlay(f"Daily Reward: +{coins_earned} Coins! [{reward_stats['rarity']}]", color, duration=3.0)
    return True

# =============================
# JSON Helpers
# =============================

def load_json_file(filename, default):
    """Lädt eine JSON-Datei oder erstellt sie mit Standardwerten.

    Wenn die Datei nicht existiert, wird sie mit den Standardwerten erstellt.
    Bei Lesefehlern werden die Standardwerte zurückgegeben.

    Args:
        filename (str): Pfad zur JSON-Datei
        default: Standarddaten, die verwendet werden sollen

    Returns:
        Daten aus der Datei oder Standardwerte
    """
    if not os.path.exists(filename):
        # Datei existiert nicht: Erstelle mit Standardwerten
        dirpath = os.path.dirname(filename)
        if dirpath:
            os.makedirs(dirpath, exist_ok=True)
        with open(filename, "w", encoding="utf-8") as f:
            json.dump(default, f, indent=4, ensure_ascii=False)
        return default
    try:
        # Datei existiert: Lade Daten
        with open(filename, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        # Fehler beim Laden: Verwende Standardwerte
        return default


def save_json_file(filename, data):
    """Speichert Daten in eine JSON-Datei."""
    dirpath = os.path.dirname(filename)
    if dirpath:
        os.makedirs(dirpath, exist_ok=True)
    with open(filename, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=4, ensure_ascii=False)


# =============================
# Data Directory
# =============================
_DATA_DIR = os.path.join(BASE_DIR, "data")
os.makedirs(_DATA_DIR, exist_ok=True)

# =============================
# Sichere Verschlüsselung des Spielstands (reine Python-Stdlib, kein Zusatzpaket nötig)
# =============================
# Verfahren: HMAC-SHA256-Zählermodus als Stromchiffre (analog AES-CTR) zur
# Verschlüsselung, kombiniert mit einem separaten HMAC-SHA256 als
# Integritäts-/Authentizitäts-Tag ("Encrypt-then-MAC"). Der Schlüssel wird per
# PBKDF2-HMAC-SHA256 (200.000 Runden) aus einem fest im Sourcecode
# hinterlegten Geheimnis abgeleitet. Ohne dieses Geheimnis (also ohne den
# JumpNDodge-Sourcecode) lässt sich die Datei nicht in lesbaren Klartext
# zurückverwandeln - jeder Entschlüsselungsversuch mit falschem Schlüssel
# schlägt an der Tag-Prüfung fehl. Jede Datei bekommt einen zufälligen Nonce,
# damit identische Inhalte nie zum gleichen Geheimtext führen.
_ENCRYPTION_SECRET = b"JumpNDodge#SaveState#Key#7f3a1c9e-52d6-4b8a-9e11-0c6f2a7d4b3e#v1"
_ENCRYPTION_SALT   = b"JumpNDodge-SaveData-KDF-Salt-v1"
_ENC_KDF_ROUNDS    = 200_000

def _derive_save_key():
    return hashlib.pbkdf2_hmac("sha256", _ENCRYPTION_SECRET, _ENCRYPTION_SALT, _ENC_KDF_ROUNDS, dklen=32)

_SAVE_KEY = _derive_save_key()

def _hmac_keystream(key, nonce, length):
    """Erzeugt einen Schlüsselstrom im HMAC-Zählermodus (wie CTR-Mode bei Blockchiffren)."""
    out = bytearray()
    counter = 0
    while len(out) < length:
        block = hmac.new(key, nonce + counter.to_bytes(8, "big"), hashlib.sha256).digest()
        out.extend(block)
        counter += 1
    return bytes(out[:length])

def _xor_bytes(a, b):
    return bytes(x ^ y for x, y in zip(a, b))

def encrypt_bytes(plaintext):
    """Verschlüsselt Rohdaten (bytes) sicher. Rückgabe: nonce(16) + tag(32) + ciphertext."""
    nonce = os.urandom(16)
    keystream = _hmac_keystream(_SAVE_KEY, nonce, len(plaintext))
    ciphertext = _xor_bytes(plaintext, keystream)
    tag = hmac.new(_SAVE_KEY, nonce + ciphertext, hashlib.sha256).digest()
    return nonce + tag + ciphertext

def decrypt_bytes(blob):
    """Entschlüsselt und verifiziert einen mit encrypt_bytes() erzeugten Blob.
    Gibt None zurück, wenn die Datei manipuliert wurde oder nicht im richtigen Format ist."""
    if not blob or len(blob) < 16 + 32:
        return None
    nonce, tag, ciphertext = blob[:16], blob[16:48], blob[48:]
    expected_tag = hmac.new(_SAVE_KEY, nonce + ciphertext, hashlib.sha256).digest()
    if not hmac.compare_digest(tag, expected_tag):
        return None  # Falscher Schlüssel oder Datei wurde manipuliert
    keystream = _hmac_keystream(_SAVE_KEY, nonce, len(ciphertext))
    return _xor_bytes(ciphertext, keystream)

def save_encrypted_json(path, data):
    """Speichert ein JSON-fähiges Objekt verschlüsselt (Base64-verpackt) auf der Festplatte."""
    json_bytes = json.dumps(data, ensure_ascii=False).encode("utf-8")
    blob = encrypt_bytes(json_bytes)
    dirpath = os.path.dirname(path)
    if dirpath:
        os.makedirs(dirpath, exist_ok=True)
    with open(path, "wb") as f:
        f.write(base64.b64encode(blob))

def load_encrypted_json(path, fallback=None):
    """Lädt und entschlüsselt eine mit save_encrypted_json() erzeugte Datei."""
    try:
        with open(path, "rb") as f:
            raw = base64.b64decode(f.read())
        plain = decrypt_bytes(raw)
        if plain is None:
            return fallback
        return json.loads(plain.decode("utf-8"))
    except Exception:
        return fallback


# =============================
# Unified & verschlüsselter Spielstand (data/gameplay.json)
# =============================
# gameplay.json und tutorial.json wurden zu EINER Datei zusammengeführt: die
# Tutorial-Daten liegen jetzt unter dem Schlüssel "_tutorial" im selben
# verschlüsselten Objekt wie Stats/Achievements/Settings/etc. Die Datei ist
# vollständig verschlüsselt (siehe oben) - ohne den Sourcecode ist sie nur
# Zufalls-Bytes in Base64.
_GAMEDATA_FILE  = os.path.join(_DATA_DIR, "gameplay.json")
_MODDED_DATA_FILE = os.path.join(_DATA_DIR, "mods", "modded_data.json")
_gamedata_cache = {}
_modded_cache   = {}

def _delete_old_plaintext_savedata():
    """Löscht alte, unverschlüsselte Spielstand-Dateien (Klartext-JSON), da mit dem
    Umstieg auf das neue verschlüsselte Format der vorherige Spielstand nicht
    mehr benötigt/übernommen wird."""
    old_paths = [
        os.path.join(BASE_DIR, "gamedata.json"),
        os.path.join(BASE_DIR, "profiles.json"),
        os.path.join(BASE_DIR, "stats.json"),
        os.path.join(BASE_DIR, "achievements.json"),
        os.path.join(BASE_DIR, "highscores.json"),
        os.path.join(BASE_DIR, "settings.json"),
        os.path.join(BASE_DIR, "powers.json"),
        os.path.join(BASE_DIR, "cosmetics.json"),
        os.path.join(_DATA_DIR, "tutorial.json"),
    ]
    for p in old_paths:
        try:
            if os.path.isfile(p):
                os.remove(p)
        except Exception:
            pass

def _ensure_gamedata_loaded():
    global _gamedata_cache
    if not _gamedata_cache:
        _delete_old_plaintext_savedata()
        _gamedata_cache = load_encrypted_json(_GAMEDATA_FILE, {})
        if not isinstance(_gamedata_cache, dict):
            _gamedata_cache = {}

def _ensure_modded_loaded():
    global _modded_cache
    if not _modded_cache:
        # Nur lesen wenn Datei existiert — nie automatisch erstellen
        if os.path.isfile(_MODDED_DATA_FILE):
            try:
                with open(_MODDED_DATA_FILE, encoding='utf-8') as f:
                    _modded_cache = json.load(f)
            except Exception:
                _modded_cache = {}
        else:
            _modded_cache = {}

def load_section(section, default):
    _ensure_gamedata_loaded()
    if section not in _gamedata_cache:
        _gamedata_cache[section] = dict(default) if isinstance(default, dict) else default
    return _gamedata_cache.get(section, default)

def save_section(section, data):
    _ensure_gamedata_loaded()
    _gamedata_cache[section] = data
    save_encrypted_json(_GAMEDATA_FILE, _gamedata_cache)
    save_current_profile()

def load_modded_section(section, default):
    _ensure_modded_loaded()
    if section not in _modded_cache:
        _modded_cache[section] = dict(default) if isinstance(default, dict) else default
    return _modded_cache.get(section, default)

def save_modded_section(section, data):
    _modded_cache[section] = data
    save_json_file(_MODDED_DATA_FILE, _modded_cache)


# =============================
# Profile System (stored inside gameplay.json under "_profiles")
# =============================

current_profile_name = None
_profile_save_lock = False
online_token = None  # geheimer Pro-Profil-Token fürs Online-Leaderboard, siehe ensure_online_token()
redeemed_codes_local = []  # lokale Kopie der schon eingelösten Redeem-Codes (Grossbuchstaben), siehe redeem_code_loop()
friends_local = []  # Namen, die dieses Profil als "Freund" markiert hat - rein lokal/client-seitig,
                    # da es keine Server-Validierung braucht (reine Anzeige-Präferenz, kein Anti-Cheat-Thema)

def _load_profiles_file():
    _ensure_gamedata_loaded()
    return _gamedata_cache.get("_profiles", {"active": None, "profiles": {}})

def _save_profiles_file(pdata):
    _gamedata_cache["_profiles"] = pdata
    save_encrypted_json(_GAMEDATA_FILE, _gamedata_cache)

def get_all_profiles():
    return _load_profiles_file().get("profiles", {})

def save_current_profile():
    global _profile_save_lock, current_profile_name
    if not current_profile_name or _profile_save_lock:
        return
    _profile_save_lock = True
    try:
        pdata = _load_profiles_file()
        pdata.setdefault("profiles", {})
        pdata["active"] = current_profile_name
        pdata["profiles"][current_profile_name] = {
            "name": current_profile_name,
            "stats": stats,
            "achievements": achievements,
            "highscores": saved_scores,
            "settings": settings,
            "powers": powers_state,
            "cosmetics": cosmetics_state,
            "online_token": online_token,
            "redeemed_codes": redeemed_codes_local,
            "friends": friends_local,
        }
        _save_profiles_file(pdata)
    finally:
        _profile_save_lock = False

def load_profile(name):
    global current_profile_name, stats, achievements, saved_scores, online_token, redeemed_codes_local, friends_local
    global settings, powers_state, cosmetics_state, language_index, sound_enabled
    pdata = _load_profiles_file()
    p = pdata.get("profiles", {}).get(name)
    if p is None:
        return False
    current_profile_name = name
    stats           = {**default_stats,        **p.get("stats", {})}
    achievements    = {**default_achievements, **p.get("achievements", {})}
    saved_scores    = {**default_highscores,   **p.get("highscores", {})}
    best_times      = {**default_best_times,   **p.get("best_times", {})}
    settings        = {**default_settings,     **p.get("settings", {})}
    powers_state    = {**default_powers,       **p.get("powers", {})}
    cosmetics_state = {**default_cosmetics,    **p.get("cosmetics", {})}
    online_token    = p.get("online_token")
    redeemed_codes_local = list(p.get("redeemed_codes", []))
    friends_local = list(p.get("friends", []))
    _migrate_cosmetics_equipped_player(cosmetics_state)
    powers_state, _powers_changed = _strip_unknown_powers(powers_state)
    _gamedata_cache.update({
        "stats": stats, "achievements": achievements, "highscores": saved_scores,
        "best_times": best_times, "settings": settings, "powers": powers_state, "cosmetics": cosmetics_state,
    })
    if _powers_changed:
        save_current_profile()
    global difficulty_index, current_game_mode, two_player_submode, klassik_mode
    lang = settings.get("language", "Deutsch")
    if lang in languages:
        language_index = languages.index(lang)
    sound_enabled = settings.get("sound_enabled", True)
    current_game_mode  = settings.get("game_mode", "standard")
    two_player_submode = settings.get("2player_submode", "coop")
    klassik_mode = (current_game_mode == "klassik")
    # Load the difficulty saved for the current mode
    _dpm = settings.get("difficulty_per_mode", {})
    diff = _dpm.get(current_game_mode, settings.get("difficulty", "Mittel"))
    settings["difficulty"] = diff
    if diff in difficulties:
        difficulty_index = difficulties.index(diff)
    return True

def create_profile(name):
    global current_profile_name, stats, achievements, saved_scores, best_times
    global settings, powers_state, cosmetics_state, online_token, redeemed_codes_local, friends_local
    current_profile_name = name
    stats           = dict(default_stats)
    achievements    = dict(default_achievements)
    saved_scores    = dict(default_highscores)
    best_times      = dict(default_best_times)
    settings        = dict(default_settings)
    powers_state    = dict(default_powers)
    cosmetics_state = dict(default_cosmetics)
    online_token    = None  # wird bei Bedarf von ensure_online_token() erzeugt
    redeemed_codes_local = []
    friends_local = []
    _gamedata_cache.update({
        "stats": stats, "achievements": achievements, "highscores": saved_scores,
        "best_times": best_times, "settings": settings, "powers": powers_state, "cosmetics": cosmetics_state,
    })
    save_current_profile()

def delete_profile(name):
    pdata = _load_profiles_file()
    profile = pdata.get("profiles", {}).pop(name, None)
    if pdata.get("active") == name:
        pdata["active"] = None
    if pdata.get("auto_start") == name:
        pdata["auto_start"] = None  # gelöschtes Profil kann kein Auto-Start-Ziel mehr sein
    token = (profile or {}).get("online_token")
    if token:
        # Account soll auch online verschwinden (siehe _flush_pending_online_deletions()) -
        # der Token muss VOR dem Löschen des Profils gesichert werden, sonst ist er nach
        # dem pop() oben weg. In _pending_online_deletions merken, bis ein Server-Kontakt
        # (jetzt oder beim nächsten Programmstart) das Löschen bestätigt.
        pending = pdata.setdefault("_pending_online_deletions", [])
        if token not in pending:
            pending.append(token)
    _save_profiles_file(pdata)
    if token:
        _flush_pending_online_deletions()

def get_auto_start_profile():
    """Gibt den Namen des Profils zurück, mit dem das Spiel automatisch starten
    soll (ohne Profilauswahl-Bildschirm), oder None wenn keins gesetzt ist.
    Liegt bewusst NICHT in den Profil-Settings selbst, sondern eine Ebene höher
    in der Profildatei - muss ja gelesen werden können, BEVOR überhaupt ein
    Profil geladen wurde."""
    return _load_profiles_file().get("auto_start")

def set_auto_start_profile(name):
    """Setzt (oder löscht mit name=None) das Profil, das beim nächsten
    Programmstart automatisch geladen wird, ohne den Auswahlbildschirm zu zeigen."""
    pdata = _load_profiles_file()
    pdata["auto_start"] = name
    _save_profiles_file(pdata)


# =============================
# Tutorial-Daten: gehören jetzt als Sektion "_tutorial" zur verschlüsselten
# gameplay.json (siehe save_section()/load_section() weiter oben).
# =============================

def save_tutorial_data(data):
    save_section("_tutorial", data)

def load_tutorial_data(fallback=None):
    result = load_section("_tutorial", fallback if fallback is not None else {})
    return result if result else fallback


# =============================
# Sound System
# =============================

def _discover_ingame_music_tracks(base_path):
    """Findet alle ingame_music_<name>.mp3 im Sound-Ordner. <name> wird direkt als Track-Key
    und (lesbar aufbereitet) als Anzeige-Name verwendet - z.B. ingame_music_tuff.mp3 -> Tuff."""
    prefix, suffix = "ingame_music_", ".mp3"
    tracks = []
    try:
        filenames = sorted(os.listdir(base_path))
    except OSError:
        filenames = []
    for fn in filenames:
        if fn.lower().startswith(prefix) and fn.lower().endswith(suffix):
            name = fn[len(prefix):-len(suffix)]
            if name:
                display = name.replace("_", " ").title()
                tracks.append((name, display, fn))
    return tracks


def load_sound_theme():
    """Lädt alle Sound-Dateien und Musik-Pfade.

    Sucht im 'sound'-Verzeichnis nach den erforderlichen Dateien.
    Bei Fehlern werden None-Werte gesetzt und Warnungen ausgegeben.
    """
    global menu_click_sound, click_sound, click_back_sound, hit_sound, gameover_sound, heartbeat_sound, oneup_sound, menu_music_path, ingame_music_path, ingame_music_paths, INGAME_MUSIC_TRACKS
    # Suche sound/ zuerst in data/, dann direkt in BASE_DIR (Fallback für alte Installs)
    base_path = os.path.join(_DATA_DIR, "sound")
    if not os.path.isdir(base_path):
        base_path = os.path.join(BASE_DIR, "sound")

    def load_sound(filename):
        """Lädt eine einzelne Sound-Datei.

        Args:
            filename (str): Name der Sound-Datei

        Returns:
            pygame.mixer.Sound oder None bei Fehler
        """
        path = os.path.join(base_path, filename)
        if os.path.exists(path):
            try:
                return pygame.mixer.Sound(path)
            except Exception as e:
                print(f"[WARN] Konnte Sound {path} nicht laden: {e}")
                return None
        else:
            print(f"[WARN] Sound-Datei nicht gefunden: {path}")
            return None

    def get_path(filename):
        """Gibt den Pfad zu einer Musik-Datei zurück, falls sie existiert.

        Args:
            filename (str): Name der Musik-Datei

        Returns:
            str oder None wenn nicht gefunden
        """
        path = os.path.join(base_path, filename)
        if os.path.exists(path):
            return path
        else:
            print(f"[WARN] Musik-Datei nicht gefunden: {path}")
            return None

    # Lade alle Sound-Effekte
    menu_click_sound = load_sound("menu_click.mp3")
    click_sound = load_sound("click.mp3")
    click_back_sound = load_sound("click_back.mp3")
    gameover_sound = load_sound("gameover.mp3")
    hit_sound = load_sound("hit.mp3")
    oneup_sound = load_sound("1up.mp3")

    # Lade Musik-Pfade
    menu_music_path = get_path("menu_music.mp3")
    INGAME_MUSIC_TRACKS = _discover_ingame_music_tracks(base_path)
    ingame_music_paths = {key: get_path(filename) for key, _label, filename in INGAME_MUSIC_TRACKS}
    chosen_track = settings.get("ingame_music_track", "standard")
    ingame_music_path = ingame_music_paths.get(chosen_track) or next(iter(ingame_music_paths.values()), None)

    # Debug-Ausgabe: Zeige geladene Sounds
    print(f"[SOUND] Geladene Sounds:")
    print(f"  menu_click_sound: {'Geladen' if menu_click_sound else 'None'}")
    print(f"  click_sound: {'Geladen' if click_sound else 'None'}")
    print(f"  click_back_sound: {'Geladen' if click_back_sound else 'None'}")
    print(f"  hit_sound: {'Geladen' if hit_sound else 'None'}")
    print(f"  gameover_sound: {'Geladen' if gameover_sound else 'None'}")
    print(f"  menu_music_path: {menu_music_path}")
    print(f"  ingame_music_path: {ingame_music_path}")

    # Heartbeat-Sound prozedural generieren (kein Datei-Load nötig)
    heartbeat_sound = _generate_heartbeat_sound()
    print(f"  heartbeat_sound: {'Generiert' if heartbeat_sound else 'Fehler'}")

def _generate_heartbeat_sound():
    """Generiert einen Lub-Dub Herzschlag-Sound prozedural."""
    import array as _array
    try:
        sr, _size, ch = pygame.mixer.get_init()
        # Two thumps: lub (strong) then dub (softer), then silence
        beat_hz = 52.0
        dur1, gap, dur2, sil = 0.10, 0.12, 0.08, 0.60
        total_samples = int(sr * (dur1 + gap + dur2 + sil))
        buf = _array.array('h', [0] * total_samples * ch)

        def _write_thump(offset, duration, amplitude):
            n = int(sr * duration)
            for i in range(min(n, total_samples - offset)):
                t = i / sr
                v = int(amplitude * exp(-t / 0.028) * sin(2 * pi * beat_hz * t))
                for c in range(ch):
                    buf[(offset + i) * ch + c] = max(-32767, min(32767, v))

        _write_thump(0, dur1, 28000)
        _write_thump(int(sr * (dur1 + gap)), dur2, 20000)
        return pygame.mixer.Sound(buffer=buf)
    except Exception as e:
        print(f"[WARN] Heartbeat konnte nicht generiert werden: {e}")
        return None

def play_sound(sound, volume_key="sfx_volume"):
    """Spielt einen Sound ab, falls Sounds aktiviert sind und der Sound existiert.

    Args:
        sound: pygame.mixer.Sound Objekt
        volume_key (str): Schlüssel für die Lautstärke-Einstellung
    """
    if sound_enabled and sound:
        user_vol = settings.get(volume_key, 0.8)  # Benutzer-Lautstärke (0-2)
        pygame_vol = get_pygame_volume(user_vol)  # Konvertiere zu pygame-Lautstärke (0-1)
        sound.set_volume(pygame_vol)
        sound.play()

def play_menu_music():
    """Spielt die Menü-Musik ab."""
    if sound_enabled and menu_music_path:
        try:
            pygame.mixer.music.load(menu_music_path)
            user_vol = settings.get("menu_music_volume", 0.8)
            pygame_vol = get_pygame_volume(user_vol)
            pygame.mixer.music.set_volume(pygame_vol)
            pygame.mixer.music.play(-1)  # Endlosschleife
        except Exception as e:
            print(f"[WARN] Konnte Menü-Musik nicht abspielen: {e}")

def play_ingame_music():
    """Spielt die Spiel-Musik ab (aktuell in den Einstellungen gewählter Track)."""
    global ingame_music_path
    chosen_track = settings.get("ingame_music_track", "standard")
    ingame_music_path = ingame_music_paths.get(chosen_track) or next(iter(ingame_music_paths.values()), None)
    if sound_enabled and ingame_music_path:
        try:
            pygame.mixer.music.load(ingame_music_path)
            user_vol = settings.get("ingame_music_volume", 0.8)
            pygame_vol = get_pygame_volume(user_vol)
            pygame.mixer.music.set_volume(pygame_vol)
            pygame.mixer.music.play(-1)  # Endlosschleife
        except Exception as e:
            print(f"[WARN] Konnte Spiel-Musik nicht abspielen: {e}")

def stop_music():
    """Stoppt die aktuelle Musik."""
    pygame.mixer.music.stop()


def preview_ingame_music_track(track_key, duration_ms=5000):
    """Spielt bis zu `duration_ms` (Standard 5s) einer Ingame-Musik-Variante zum Vorhören ab,
    unabhängig von der eigentlichen Musik (eigener Kanal, unterbricht nicht die Menü-Musik).
    Ein Klick auf einen anderen Track stoppt die laufende Preview sofort und startet die neue.
    Ein Klick auf den gerade laufenden Track stoppt ihn nur (Toggle)."""
    global _preview_channel, _preview_track_key
    was_same = _preview_track_key == track_key
    if _preview_channel is not None:
        _preview_channel.stop()
        _preview_channel = None
        _preview_track_key = None
        if was_same:
            return

    path = ingame_music_paths.get(track_key)
    if not (sound_enabled and path):
        return
    try:
        sound = pygame.mixer.Sound(path)
        sound.set_volume(get_pygame_volume(settings.get("ingame_music_volume", 0.8)))
        _preview_channel = sound.play(loops=0, maxtime=duration_ms)
        _preview_track_key = track_key
    except Exception as e:
        print(f"[WARN] Konnte Musik-Vorschau nicht abspielen: {e}")


def is_previewing_track(track_key):
    """True während der Vorhör-Preview für `track_key` noch läuft. Räumt automatisch auf,
    sobald die Preview von selbst ausläuft (nach duration_ms), damit die UI (Play/Stop-Icon)
    ohne weiteren Klick wieder zurückspringt."""
    global _preview_channel, _preview_track_key
    if _preview_channel is not None and not _preview_channel.get_busy():
        _preview_channel = None
        _preview_track_key = None
    return _preview_track_key == track_key


def get_pygame_volume(user_volume):
    """Konvertiert Benutzer-Lautstärke (0-2) zu pygame-Lautstärke (0-1) mit besserer Tiefenkontrolle.

    Die Lautstärke wird nicht linear abgebildet, um bei niedrigen Werten
    eine bessere Kontrolle zu haben.

    Args:
        user_volume (float): Benutzer-Lautstärke (0.0 bis 2.0)

    Returns:
        float: pygame-Lautstärke (0.0 bis 1.0)
    """
    if user_volume <= 1.0:
        return user_volume * 0.1  # 0.0 -> 0.0, 1.0 -> 0.1 (leiser Bereich)
    else:
        return 0.1 + (user_volume - 1.0) * 0.9  # 1.0 -> 0.1, 2.0 -> 1.0 (lauter Bereich)


def toggle_sound_enabled():
    """Schaltet Sounds ein/aus."""
    global sound_enabled
    sound_enabled = not sound_enabled
    settings["sound_enabled"] = sound_enabled
    save_section("settings", settings)
    if not sound_enabled:
        stop_music()  # Stoppe Musik wenn Sounds deaktiviert werden


def adjust_menu_click_volume(delta):
    """Passt die Lautstärke für Menü-Klick-Sounds an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("menu_click_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["menu_click_volume"] = new
    save_section("settings", settings)


def adjust_click_volume(delta):
    """Passt die Lautstärke für allgemeine Klick-Sounds an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("click_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["click_volume"] = new
    save_section("settings", settings)


def adjust_click_back_volume(delta):
    """Passt die Lautstärke für Zurück-Klick-Sounds an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("click_back_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["click_back_volume"] = new
    save_section("settings", settings)


def adjust_hit_volume(delta):
    """Passt die Lautstärke für Treffer-Sounds an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("hit_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["hit_volume"] = new
    save_section("settings", settings)


def adjust_gameover_volume(delta):
    """Passt die Lautstärke für Game-Over-Sounds an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("gameover_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["gameover_volume"] = new
    save_section("settings", settings)


def adjust_menu_music_volume(delta):
    """Passt die Lautstärke für Menü-Musik an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("menu_music_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["menu_music_volume"] = new
    save_section("settings", settings)
    if pygame.mixer.music.get_busy():  # Wenn Musik läuft, sofort anwenden
        pygame_vol = get_pygame_volume(new)
        pygame.mixer.music.set_volume(pygame_vol)


def adjust_ingame_music_volume(delta):
    """Passt die Lautstärke für Spiel-Musik an.

    Args:
        delta (float): Änderung der Lautstärke (-0.1 oder +0.1)
    """
    current = settings.get("ingame_music_volume", 0.8)
    new = max(0.0, min(2.0, current + delta))  # Begrenze auf 0.0-2.0
    settings["ingame_music_volume"] = new
    save_section("settings", settings)
    if pygame.mixer.music.get_busy():  # Wenn Musik läuft, sofort anwenden
        pygame_vol = get_pygame_volume(new)
        pygame.mixer.music.set_volume(pygame_vol)


# =============================
# Defaults & Persistent Data
# =============================

# Standardwerte für Spielstatistiken
default_stats = {
    "total_time_played": 0,    # Gesamte Spielzeit in Sekunden
    "total_rounds": 0,         # Anzahl gespielter Runden
    "total_1ups": 0,           # Anzahl gesammelter 1-UP-Powerups
    "coins": 0,                # Anzahl gesammelter Coins
    "tutorial_seen": False,    # Tutorial beim ersten Start angezeigt?
    "total_jumps": 0,          # Anzahl aller Sprünge (inkl. Doppelsprung)
    "total_deaths": 0,         # Anzahl Game Overs
    "total_coins_earned": 0,   # Lifetime verdiente Coins (unabhängig vom aktuellen Kontostand)
    "score_sum_by_diff": {},   # Summe aller Endscores pro Schwierigkeit (für Ø Score/Runde)
    "rounds_by_diff": {}       # Anzahl Runden pro Schwierigkeit (für Ø Score/Runde)
}

# Standardwerte für Erfolge (leer, werden dynamisch hinzugefügt)
default_achievements = {}

# Standardwerte für Highscores pro Schwierigkeitsgrad
default_highscores = {
    "Leicht": 0, "Mittel": 0, "Schwer": 0, "Hardcore": 0, "Unmöglich": 0, "Albtraum": 0, "Hölle": 0, "Dynamisch": 0
}

# Separate Highscores und Stats für Modded-Runs
default_modded_scores = {
    "Leicht": 0, "Mittel": 0, "Schwer": 0, "Hardcore": 0, "Unmöglich": 0, "Albtraum": 0, "Hölle": 0, "Dynamisch": 0
}
default_best_times = {
    "Leicht": 0, "Mittel": 0, "Schwer": 0, "Hardcore": 0, "Unmöglich": 0, "Albtraum": 0, "Hölle": 0, "Dynamisch": 0
}
default_modded_best_times = {
    "Leicht": 0, "Mittel": 0, "Schwer": 0, "Hardcore": 0, "Unmöglich": 0, "Albtraum": 0, "Hölle": 0, "Dynamisch": 0
}
default_modded_stats = {
    "total_time_played": 0,
    "total_rounds": 0,
    "total_1ups": 0,
    "coins": 0,
}

# Standardwerte für Spieleinstellungen
default_settings = {
    "language": "English",                    # Standardsprache
    "difficulty": "Mittel",                   # Standardschwierigkeit
    "fullscreen": False,                      # Vollbildmodus standardmäßig aus
    "music_volume": 0.8,                      # Musik-Lautstärke (0-2)
    "sfx_volume": 0.8,                        # Soundeffekt-Lautstärke (0-2)
    "dark_mode": True,                        # Dark Mode standardmäßig an
    "sound_enabled": True,                    # Sounds standardmäßig aktiviert
    "keybindings": {                                             # Steuerung (als pygame-Key-Codes)
        "jump":   [pygame.K_SPACE, pygame.K_w, pygame.K_UP],
        "left":   [pygame.K_LEFT,  pygame.K_a],
        "right":  [pygame.K_RIGHT, pygame.K_d],
        "power1": [pygame.K_q],
        "power2": [pygame.K_e],
    },
    "menu_click_volume": 0.8,                 # Menü-Klick-Lautstärke
    "click_volume": 0.8,                      # Klick-Lautstärke
    "click_back_volume": 0.8,                 # Zurück-Klick-Lautstärke
    "hit_volume": 0.8,                        # Treffer-Lautstärke
    "gameover_volume": 0.8,                   # Game-Over-Lautstärke
    "menu_music_volume": 0.8,                 # Menü-Musik-Lautstärke
    "ingame_music_volume": 0.8,               # Spiel-Musik-Lautstärke
    "ingame_music_track": "standard",         # Gewählter Ingame-Musik-Track (siehe INGAME_MUSIC_TRACKS)
    "show_floor_line": True,                  # Violette Boden-Linie während des Spiels
    "game_mode": "standard",                  # Aktiver Spielmodus: "standard" | "klassik" | "2player"
    "difficulty_per_mode": {                  # Gespeicherte Schwierigkeit pro Modus
        "standard": "Mittel",
        "klassik":  "Mittel",
        "2player":  "Mittel"
    },
    "2player_submode": "coop",                # 2-Player-Submodus: "coop" | "vs"
    "custom_speed": 10,                       # Custom-Modus: Geschwindigkeit
    "custom_min_delay": 900,                  # Custom-Modus: minimale Hindernis-Pause (ms)
    "custom_max_delay": 1600,                 # Custom-Modus: maximale Hindernis-Pause (ms)
    "custom_oneups": True,                    # Custom-Modus Modifier: 1-Ups aktiv
    "custom_vertical_obstacles": False,        # Custom-Modus Modifier: vertikale (fallende) Hindernisse
    "custom_wind_sides": False,                 # Custom-Modus Modifier: Wind nur von den Seiten (horizontal)
    "custom_wind_all": False,                   # Custom-Modus Modifier: Wind von allen Seiten (horizontal + vertikal)
    "custom_free_roam": False,                  # Custom-Modus Modifier: freie WASD-Bewegung, Hindernisse aus allen Richtungen
}

# Standardwerte für Powers (Fähigkeiten)
default_powers = {
    "owned": [],     # Liste der besessenen Powers
    "equipped": []   # Liste der ausgerüsteten Powers
}

# Standardwerte für Kosmetik-Items
default_cosmetics = {
    "owned": [],               # Liste der besessenen Kosmetik-Items
    "equipped": [],            # Ausgerüstete Trails (Spuren)
    "equipped_headwear": None, # Ausgerüstete Kopfbedeckung (z.B. Krone)
    "equipped_eyes": None,     # Ausgerüstetes Augen-Item (z.B. Pixel-Augen)
    "equipped_nose": None,     # Ausgerüstetes Nasen-Item (z.B. Clown-Nase)
    "equipped_mouth": None     # Ausgerüstetes Mund-Item (z.B. Breites Grinsen)
}

def _migrate_cosmetics_equipped_player(state):
    """Alte Speicherstände hatten nur EINEN gemeinsamen Slot ('equipped_player') für
    Kopfbedeckungen UND Gesichtsmerkmale. Diese Funktion übernimmt einen dort noch
    vorhandenen Wert einmalig in den passenden neuen Kategorie-Slot, damit niemand
    sein bereits ausgerüstetes Item durch das Update verliert."""
    if not isinstance(state, dict):
        return state
    old = state.pop("equipped_player", None)
    if old:
        meta = COSMETICS_DB.get(old, {})
        category = meta.get("category")
        if category == "headwear" and not state.get("equipped_headwear"):
            state["equipped_headwear"] = old
        elif category in ("eyes", "nose", "mouth") and not state.get(f"equipped_{category}"):
            state[f"equipped_{category}"] = old
    return state

def _migrate_cosmetics_equipped_face(state):
    """Ein vorheriges Update hatte Kopfbedeckung UND Gesichtsmerkmale (Augen, Nase,
    Mund) noch in einem gemeinsamen 'equipped_face'-Slot zusammengefasst. Diese
    Funktion übernimmt einen dort noch vorhandenen Wert (z.B. Pixel-Augen) einmalig
    in den passenden neuen, getrennten Kategorie-Slot (equipped_eyes / equipped_nose /
    equipped_mouth), damit niemand sein bereits ausgerüstetes Item durch das Update
    verliert."""
    if not isinstance(state, dict):
        return state
    old = state.pop("equipped_face", None)
    if old:
        meta = COSMETICS_DB.get(old, {})
        category = meta.get("category")
        if category in ("eyes", "nose", "mouth") and not state.get(f"equipped_{category}"):
            state[f"equipped_{category}"] = old
    return state

# Lade persistente Daten aus data/gameplay.json (migriert automatisch alte Einzeldateien)
stats           = load_section("stats",          default_stats)
achievements    = load_section("achievements",  default_achievements)
saved_scores    = load_section("highscores",    default_highscores)
best_times      = load_section("best_times",   default_best_times)
modded_scores   = load_modded_section("modded_highscores", default_modded_scores)
modded_best_times = load_modded_section("modded_best_times", default_modded_best_times)
modded_stats    = load_modded_section("modded_stats",      default_modded_stats)
settings        = load_section("settings",      default_settings)
powers_state    = load_section("powers",        default_powers)
cosmetics_state = load_section("cosmetics",     default_cosmetics)
sale_state      = load_section("sale",          {"date": "", "items": {}})

set_display_mode()  # Wende den Anzeigemodus an (Vollbild/Fenster)

# Lade anfängliche Sounds und Musik
load_sound_theme()

# --- fullscreen scaling helper ---
orig_flip = pygame.display.flip

# =============================
# Fade Transition System
# =============================
_fade_alpha    = 0
_fade_dir      = 0
_fade_callback = None
_FADE_SPEED    = 22

def request_fade(callback):
    global _fade_alpha, _fade_dir, _fade_callback
    _fade_alpha    = 0
    _fade_dir      = 1
    _fade_callback = callback

_window_locked_for_gameplay = False  # Spiegelt, ob das Fenster gerade (wegen game_running) nicht-resizebar ist

def sync_window_size():
    """Muss am Anfang jedes Frames aufgerufen werden — updated WIDTH/HEIGHT vor dem Draw.

    Während einer laufenden Runde (game_running) wird das Fenster im Fenstermodus auf
    nicht-resizebar umgeschaltet, damit es während der Runde gar nicht erst per Maus in der
    Größe geändert werden kann - vorher konnte ein Live-Resize Boden/Wände verschieben und
    dadurch Spieler und Hindernisse springen lassen. Die Umschaltung (pygame.display.set_mode
    mit/ohne RESIZABLE-Flag) passiert nur EINMAL beim Wechsel, erkannt über den Vergleich mit
    _window_locked_for_gameplay - game_running wird an vielen Stellen im Code gesetzt (Pause/
    Resume/Rematch usw.), die dafür nicht einzeln angefasst werden müssen. Sobald die Runde
    vorbei ist, wird die Resizebarkeit automatisch wiederhergestellt."""
    global base_surface, WIDTH, HEIGHT, scale_x, scale_y, offset_x, offset_y, display_surface, _window_locked_for_gameplay
    global _submit_pending_local_name
    try:
        _process_dev_commands()
    except Exception:
        pass
    if _submit_pending_local_name is not None:
        rename_current_profile_local(_submit_pending_local_name)
        _submit_pending_local_name = None
    _apply_confirmed_online_deletions()
    try:
        fullscreen = settings.get("fullscreen", False)
        if not fullscreen and game_running != _window_locked_for_gameplay:
            flags = 0 if game_running else pygame.RESIZABLE
            display_surface = pygame.display.set_mode((WIDTH, HEIGHT), flags)
            _window_locked_for_gameplay = game_running

        win_w = display_surface.get_width()
        win_h = display_surface.get_height()
        if not fullscreen:
            win_w = max(win_w, 400)
            win_h = max(win_h, 300)
        gui_scale = 1.0 if game_running else max(0.5, min(2.0, settings.get("gui_scale", 1.0)))
        logical_w = max(400, int(win_w / gui_scale))
        logical_h = max(300, int(win_h / gui_scale))
        if logical_w != WIDTH or logical_h != HEIGHT:
            WIDTH = logical_w
            HEIGHT = logical_h
            base_surface = pygame.Surface((WIDTH, HEIGHT))
        scale_x = win_w / WIDTH
        scale_y = win_h / HEIGHT
        offset_x = 0
        offset_y = 0
    except Exception:
        pass


def _draw_modded_badge():
    """Draw a MODDED badge directly onto display_surface (top-right corner).

    Draws in display_surface pixel space so it works correctly regardless
    of base_surface scaling. Called inside the patched pygame.display.flip().
    """
    try:
        if not mod_system.mods:
            return
        n_active = mod_system.get_active_count()
        n_broken = sum(1 for m in mod_system.mods if m.error is not None)
        label = f"MODDED ({n_active})"
        if n_broken:
            label += f"  {n_broken} ERR"
        badge_surf = small_font.render(label, True, (255, 200, 0))
        bw = badge_surf.get_width() + 12
        bh = badge_surf.get_height() + 6
        dw = display_surface.get_width()
        bx = dw - bw - 4
        by = 4
        badge_bg = pygame.Surface((bw, bh), pygame.SRCALPHA)
        badge_bg.fill((0, 0, 0, 160))
        pygame.draw.rect(badge_bg, (180, 120, 0), badge_bg.get_rect(), width=1, border_radius=5)
        display_surface.blit(badge_bg, (bx, by))
        display_surface.blit(badge_surf, (bx + 6, by + 3))
    except Exception:
        pass


def _jnd_flip():
    global _fade_alpha, _fade_dir, _fade_callback
    try:
        dw = display_surface.get_width()
        dh = display_surface.get_height()
        if base_surface.get_width() != dw or base_surface.get_height() != dh:
            scaled = pygame.transform.smoothscale(base_surface, (dw, dh))
            display_surface.blit(scaled, (0, 0))
        else:
            display_surface.blit(base_surface, (0, 0))
    except Exception:
        pass

    if _fade_dir != 0 or _fade_alpha > 0:
        _fade_alpha += _fade_dir * _FADE_SPEED
        if _fade_dir == 1 and _fade_alpha >= 255:
            _fade_alpha = 255
            if _fade_callback:
                _fade_callback()
                _fade_callback = None
            _fade_dir = -1
        elif _fade_dir == -1 and _fade_alpha <= 0:
            _fade_alpha = 0
            _fade_dir   = 0
        if _fade_alpha > 0:
            try:
                sw = display_surface.get_width()
                sh = display_surface.get_height()
                fade_surf = pygame.Surface((sw, sh), pygame.SRCALPHA)
                fade_surf.fill((0, 0, 0, min(255, max(0, _fade_alpha))))
                display_surface.blit(fade_surf, (0, 0))
            except Exception:
                pass

    orig_flip()

pygame.display.flip = _jnd_flip

# =============================
# Localized Texts (loadout -> inventory)
# Cosmetic texts
# =============================
texts = {
    "Deutsch": {
        "ice_trail": "Eisspur",
        "shadow_trail": "Schattenspur",
        "sparkle_trail": "Funkelspur",
        "toxic_trail": "Giftspur",
        "start": "Start", "language": "Sprache", "difficulty": "Schwierigkeit",
        "game_over": "Spiel vorbei!", "retry": "Neustart",
        "score": "Punkte", "lives": "Leben", "highscore": "Rekord", "stats": "Statistiken",
        "back": "Zurück", "stat_highscores": "Highscores", "stat_achievements": "Erfolge", "stat_total": "Gesamt", "stat_modded": "Modded",
        "no_achievements": "Noch keine Erfolge freigeschaltet.", "total_time": "Gesamte Spielzeit",
        "rounds_played": "Gespielte Runden", "1ups_collected": "1-Ups gesammelt", "record": "Rekord", "current": "Aktuell",
        "table_difficulty": "Schwierigkeit", "table_highscore": "Rekord", "table_session": "Sitzung",
        "online_leaderboard_btn": "Online-Rangliste", "search": "Suche", "no_entries_yet": "Noch keine Einträge",
        "player_search_title": "Spielersuche", "player_not_found": "Spieler nicht gefunden", "powers_used_label": "Verwendete Powers",
        "owned_powers_label": "Freigeschaltete Powers", "owned_cosmetics_label": "Freigeschaltete Cosmetics",
        "diff_Leicht": "Leicht", "diff_Mittel": "Mittel", "diff_Schwer": "Schwer", "diff_Hardcore": "Hardcore",
        "diff_Unmöglich": "Unmöglich", "diff_Albtraum": "Albtraum", "diff_Hölle": "Hölle", "diff_Dynamisch": "Dynamisch",
        "redeem_key_btn": "Verified-Key einlösen", "rename_btn": "Namen ändern",
        "redeem_key_success": "Key erfolgreich eingelöst!", "rename_success": "Name geändert!",
        "key_invalid_msg": "Ungültiger Key", "key_revoked_msg": "Dieser Key wurde gesperrt",
        "key_shared_msg": "Key wurde von jemandem geteilt - beide Nutzungen gesperrt",
        "unknown_player_msg": "Bitte zuerst eine Standard-Runde spielen", "play_time_label": "Spielzeit",
        "generic_error_msg": "Etwas ist schiefgelaufen - Server nicht erreichbar oder Update ausstehend",
        "redeem_code_btn": "Code einlösen", "redeem_code_success": "Code eingelöst!",
        "code_invalid_msg": "Ungültiger Code", "code_already_redeemed_msg": "Code bereits eingelöst",
        "paste_btn": "Einfügen", "add_friend_btn": "+ Freund", "remove_friend_btn": "- Freund",
        "friends_hub_title": "Freunde", "friends_tab_roster": "Meine Freunde", "friends_tab_requests": "Anfragen",
        "no_requests_yet": "Keine Anfragen", "accept_btn": "Annehmen", "decline_btn": "Ablehnen",
        "message_btn": "Nachricht", "friend_request_sent_msg": "Anfrage gesendet!", "already_friends_msg": "Ihr seid bereits befreundet",
        "clan_hub_title": "Clan", "create_clan_btn": "Clan erstellen", "join_clan_btn": "Clan beitreten",
        "leave_clan_btn": "Verlassen", "kick_btn": "Kick", "clan_tag_label": "Tag", "clan_name_label": "Name",
        "clan_not_in_one": "Du bist in keinem Clan", "clan_members_label": "Mitglieder", "clan_owner_label": "Anführer",
        "clan_search_title": "Clan-Suche", "clan_not_found": "Clan nicht gefunden",
        "clan_fields_required_msg": "Bitte Tag und Namen eingeben", "clan_already_in_one_msg": "Du bist bereits in einem Clan",
        "clan_tag_taken_msg": "Tag bereits vergeben", "clan_name_taken_msg": "Name bereits vergeben", "clan_full_msg": "Clan ist voll",
        "chat_title": "Chat", "send_btn": "Senden", "no_messages_yet": "Noch keine Nachrichten",
        "chat_message_too_long_msg": "Nachricht zu lang", "chat_message_filtered_msg": "Nachricht nicht erlaubt",
        "chat_not_friends_msg": "Ihr seid nicht befreundet", "chat_not_clan_member_msg": "Du bist nicht in diesem Clan",
        "chat_rate_limited_msg": "Bitte kurz warten",
        "friends_tab": "Freunde", "all_tab_lb": "Alle", "no_friends_yet": "Noch keine Freunde markiert",
        "total_jumps": "Sprünge gesamt", "total_deaths": "Game Overs", "avg_score": "Ø Score", "total_coins_earned": "Coins gesamt verdient",
        "shop": "Shop", "loadout": "Inventar", "coins": "Coins",
        "power_roll": "Power Roll",
        "rollen": "Rollen",
        "coming_soon": "Bald verfügbar!",
        "admin": "Admin", "enter_password": "Admin-Passwort:", "wrong_password": "Falsches Passwort",
        "add_coins": "Coins hinzufügen", "set_coins": "Coins setzen", "amount": "Menge",
        "logout": "Abmelden", "give_power": "Power geben",
        "spin": "Drehen (50)", "spin10": "Roll x10 (500)", "results": "Belohnungen",
        "owned_tab": "Besitzt", "equipped_tab": "Ausgerüstet", "all_tab": "Alle", "no_owned": "Keine Powers besessen.", "more": "Mehr", "help": "Anleitung",
        "classic_mode": "JumpNDodge Classic", "esc_back": "ESC: Zurück",
        "game_subtitle": "Ein actionreiches Sprungspiel", "controls_hint": "ENTER: Start  |  W/S: Sprache/Schwierigkeit  |  ESC: Beenden",
        "back_to_main": "ESC/M: Zurück zum Hauptmenü", "click_equip": "Klick: Equip (Auto-Swap wenn voll)  |  ESC/M: Menü",
        "locked": "gesperrt", "equipped": "ausgerüstet", "passive": "passiv", "auto_swap": "Auto-Swap", "new_power": "Neu",
        "duplicate": "Duplikat", "extra_slot_unlocked": "Extra Slot freigeschaltet! (passiv)", "power_added": "Power hinzugefügt!",
        "power_already_owned": "Power bereits besessen", "protected": "Geschützt!", "double_jump": "Double Jump!",
        "slow_motion": "Slow Motion!", "time_freeze": "Time Freeze!", "ghost_mode": "Ghost Mode!", "nuke": "NUKE!",
        "1up_collected": "1-UP eingesammelt", "1up": "1-UP!", "insufficient_coins": "Zu wenig Coins!",
        "roll_x10_complete": "Roll x10 abgeschlossen", "roll_x10_coins": "Roll x10: +{0} Coins zurück",
        "description": "Beschreibung", "not_owned": "Nicht besessen",
        "daily_reward": "Tägliche Belohnung", "daily_reward_claimed": "Bereits erhalten",
        "daily_reward_no_internet": "Kein Internet", "daily_reward_available": "Verfügbar",
        "last_reward": "Letzte Belohnung", "reset_time": "Reset: 00:00 Uhr",
        "dark_mode": "Dark Mode", "light_mode": "Light Mode", "protected": "Geschützt!", "new_power": "Neue Power",
        "cosmetic_inventory": "Kosmetik-Inventar",
        "cosmetic_shop": "Kosmetik-Shop",
        "trails_tab": "Spuren",
        "player_cosmetics_tab": "Spieler Kosmetik",
        "headwear_tab": "Kopfbedeckungen",
        "face_tab": "Gesichtsmerkmale",
        "eyes_tab": "Augen",
        "nose_tab": "Nase",
        "mouth_tab": "Mund",
        "tools_tab": "Tools",
        "tools_coming_soon": "Bald verfügbar!",
        "sale_view_title": "Heutiges Angebot", "sale_next_in": "Nächstes Angebot in:", "sale_no_items": "Heute kein Angebot verfügbar.",
        "daily_rewards": "Tägliche Belohnungen", "all_statistics": "Alle Statistiken", "difficulties": "Schwierigkeiten",
        "loading": "Lädt...", "claim_reward": "Belohnung abholen", "next_reset_in": "Nächster Reset in:", "achievement_unlocked": "Erfolg: ", "settings": "Einstellungen", "export_language": "Sprache exportieren", "sound": "Sound", "sound_enabled": "Sound aktiviert", "sound_theme": "Sound-Thema", "menu_click_label": "Menü-Klick", "click_label": "Klick", "click_back_label": "Zurück-Klick", "hit_label": "Treffer", "gameover_label": "Game Over", "menu_music_label": "Menü-Musik", "ingame_music_label": "Spiel-Musik", "on": "AN", "off": "AUS",
        "choose_language": "Wähle deine Sprache", "color_scheme": "Farbschema", "lang_continue": "Weiter",
        "rarity_label": "Seltenheit", "rarity_normal": "Normal", "rarity_rare": "Selten", "rarity_epic": "Episch",
        "achievements": "Erfolge",
        "profiles": "Profile", "add_profile": "Profil hinzufügen", "who_is_playing": "Wer spielt?",
        "enter_name": "Wie heißt du?", "confirm": "Bestätigen", "delete_profile": "Löschen",
        "name_taken": "Name bereits vergeben!", "name_empty": "Bitte einen Namen eingeben!", "name_filtered": "Name nicht erlaubt!",
        "auto_start_profile": "Auto-Start",
        "auto_start_hint": "Häkchen = immer mit diesem Profil starten.",
        "fire_trail": "Feuerspur",
        "yellow_crown": "Gelbe Krone",
        "buy": "Kaufen", "owned": "Besessen",
        "choose_difficulty_title": "Schwierigkeit wählen", "selected_badge": "Ausgewählt", "press_a_key": "Taste drücken...",
        "no_coins_badge": "Keine Coins", "pts_per_coin": "{0} Punkte = 1 Coin",
        "scroll_back_hint": "↑/↓: Scroll   ESC/M: Zurück", "future_version_notice": "Kommt in einer zukünftigen Version.",
        "achievements_disabled_notice": "Erfolge sind momentan deaktiviert.",
        "enter_click_close_hint": "ENTER / Klick: Schließen", "enter_click_esc_close_hint": "ENTER/Klick/ESC/M: Schließen",
        "enter_login_menu_hint": "ENTER: Login  |  ESC/M: Menü", "enter_confirm_menu_hint": "ENTER: Bestätigen  |  ESC/M: Menü",
        "esc_menu_hint": "ESC: Menü", "item_acquired": "{0} erhalten!",
        "vs_mode_label": "VS", "coop_mode_label": "Koop",
        "profile_hub_title": "Profil", "change_cosmetics": "Kosmetik ändern", "switch_profile": "Profil wechseln",
        "gui_size_label": "GUI Größe:", "recommended_label": "Empfohlen", "background_label": "Hintergrund:",
        "bg_stars_toggle": "Sterne", "bg_rings_toggle": "Ringe", "bg_animated_toggle": "Animiert", "bg_mono_toggle": "Mono", "bg_floor_toggle": "Boden",
        "controls_menu_btn": "Steuerung", "reset_defaults": "Standard wiederherstellen",
        "controls_title": "Steuerung anpassen", "controls_hint_legend": "Klicke eine Taste zum Entfernen  ·  [+] zum Hinzufügen  ·  ⚠ = Taste auch anderswo belegt",
        "controls_scroll_hint": "↑/↓ oder Mausrad: Scrollen",
        "controls_tab_standard": "Standard", "controls_tab_2p": "2-Spieler", "controls_tab_menu": "Menü",
    },
    "English": {
        "ice_trail": "Ice Trail",
        "shadow_trail": "Shadow Trail",
        "sparkle_trail": "Sparkle Trail",
        "toxic_trail": "Toxic Trail",
        "start": "Start", "language": "Language", "difficulty": "Difficulty",
        "game_over": "Game Over!", "retry": "Retry",
        "score": "Score", "lives": "Lives", "highscore": "Highscore", "stats": "Statistics",
        "back": "Back", "stat_highscores": "Highscores", "stat_achievements": "Achievements", "stat_total": "Overall", "stat_modded": "Modded",
        "no_achievements": "No achievements unlocked yet.", "total_time": "Total Time Played",
        "rounds_played": "Rounds Played", "1ups_collected": "1-Ups Collected", "record": "Highscore", "current": "Current",
        "table_difficulty": "Difficulty", "table_highscore": "Highscore", "table_session": "Session",
        "online_leaderboard_btn": "Online Leaderboard", "search": "Search", "no_entries_yet": "No entries yet",
        "player_search_title": "Player Search", "player_not_found": "Player not found", "powers_used_label": "Powers used",
        "owned_powers_label": "Unlocked Powers", "owned_cosmetics_label": "Unlocked Cosmetics",
        "diff_Leicht": "Easy", "diff_Mittel": "Medium", "diff_Schwer": "Hard", "diff_Hardcore": "Hardcore",
        "diff_Unmöglich": "Impossible", "diff_Albtraum": "Nightmare", "diff_Hölle": "Hell", "diff_Dynamisch": "Dynamic",
        "redeem_key_btn": "Redeem Verified Key", "rename_btn": "Change Name",
        "redeem_key_success": "Key redeemed successfully!", "rename_success": "Name changed!",
        "key_invalid_msg": "Invalid key", "key_revoked_msg": "This key has been revoked",
        "key_shared_msg": "Key was shared with someone else - both uses revoked",
        "unknown_player_msg": "Please play a Standard round first", "play_time_label": "Play time",
        "generic_error_msg": "Something went wrong - server unreachable or update pending",
        "redeem_code_btn": "Redeem Code", "redeem_code_success": "Code redeemed!",
        "code_invalid_msg": "Invalid code", "code_already_redeemed_msg": "Code already redeemed",
        "paste_btn": "Paste", "add_friend_btn": "+ Friend", "remove_friend_btn": "- Friend",
        "friends_hub_title": "Friends", "friends_tab_roster": "My Friends", "friends_tab_requests": "Requests",
        "no_requests_yet": "No requests", "accept_btn": "Accept", "decline_btn": "Decline",
        "message_btn": "Message", "friend_request_sent_msg": "Request sent!", "already_friends_msg": "You're already friends",
        "clan_hub_title": "Clan", "create_clan_btn": "Create Clan", "join_clan_btn": "Join Clan",
        "leave_clan_btn": "Leave", "kick_btn": "Kick", "clan_tag_label": "Tag", "clan_name_label": "Name",
        "clan_not_in_one": "You're not in a clan", "clan_members_label": "Members", "clan_owner_label": "Leader",
        "clan_search_title": "Clan Search", "clan_not_found": "Clan not found",
        "clan_fields_required_msg": "Please enter a tag and name", "clan_already_in_one_msg": "You're already in a clan",
        "clan_tag_taken_msg": "Tag already taken", "clan_name_taken_msg": "Name already taken", "clan_full_msg": "Clan is full",
        "chat_title": "Chat", "send_btn": "Send", "no_messages_yet": "No messages yet",
        "chat_message_too_long_msg": "Message too long", "chat_message_filtered_msg": "Message not allowed",
        "chat_not_friends_msg": "You're not friends", "chat_not_clan_member_msg": "You're not in this clan",
        "chat_rate_limited_msg": "Please wait a moment",
        "friends_tab": "Friends", "all_tab_lb": "All", "no_friends_yet": "No friends added yet",
        "total_jumps": "Total Jumps", "total_deaths": "Game Overs", "avg_score": "Avg. Score", "total_coins_earned": "Total Coins Earned",
        "shop": "Shop", "loadout": "Inventory", "coins": "Coins",
        "power_roll": "Power Roll",
        "rollen": "Rolls",
        "coming_soon": "Coming soon!",
        "admin": "Admin", "enter_password": "Admin password:", "wrong_password": "Wrong password",
        "add_coins": "Add coins", "set_coins": "Set coins", "amount": "Amount",
        "logout": "Logout", "give_power": "Give Power",
        "spin": "Spin (50)", "spin10": "Roll x10 (500)", "results": "Rewards",
        "owned_tab": "Owned", "equipped_tab": "Equipped", "all_tab": "All", "no_owned": "No powers owned.", "more": "More", "help": "Help",
        "classic_mode": "JumpNDodge Classic", "esc_back": "ESC: Back",
        "game_subtitle": "An action-packed jumping game", "controls_hint": "ENTER: Start  |  W/S: Language/Difficulty  |  ESC: Quit",
        "back_to_main": "ESC/M: Back to main menu", "click_equip": "Click: Equip (Auto-Swap when full)  |  ESC/M: Menu",
        "locked": "locked", "equipped": "equipped", "passive": "passive", "auto_swap": "Auto-Swap", "new_power": "New",
        "duplicate": "Duplicate", "extra_slot_unlocked": "Extra Slot unlocked! (passive)", "power_added": "Power added!",
        "power_already_owned": "Power already owned", "protected": "Protected!", "double_jump": "Double Jump!",
        "slow_motion": "Slow Motion!", "time_freeze": "Time Freeze!", "ghost_mode": "Ghost Mode!", "nuke": "NUKE!",
        "1up_collected": "1-UP collected", "1up": "1-UP!", "insufficient_coins": "Insufficient coins!",
        "roll_x10_complete": "Roll x10 complete", "roll_x10_coins": "Roll x10: +{0} coins back",
        "description": "Description", "not_owned": "Not owned",
        "daily_reward": "Daily Reward", "daily_reward_claimed": "Already claimed",
        "daily_reward_no_internet": "No Internet", "daily_reward_available": "Available",
        "last_reward": "Last Reward", "reset_time": "Reset: 00:00",
        "dark_mode": "Dark Mode", "light_mode": "Light Mode", "protected": "Protected!", "new_power": "New Power",
        "cosmetic_inventory": "Cosmetic Inventory",
        "cosmetic_shop": "Cosmetic Shop",
        "trails_tab": "Trails",
        "player_cosmetics_tab": "Player Cosmetics",
        "headwear_tab": "Headwear",
        "face_tab": "Face",
        "eyes_tab": "Eyes",
        "nose_tab": "Nose",
        "mouth_tab": "Mouth",
        "tools_tab": "Tools",
        "tools_coming_soon": "Coming soon!",
        "sale_view_title": "Today's Sale", "sale_next_in": "Next sale in:", "sale_no_items": "No sale available today.",
        "fire_trail": "Fire Trail",
        "yellow_crown": "Yellow Crown",
        "buy": "Buy",
        "owned": "Owned",
        "daily_rewards": "Daily Rewards", "all_statistics": "All Statistics", "difficulties": "Difficulties",
        "loading": "Loading...", "claim_reward": "Claim Reward", "next_reset_in": "Next reset in:", "achievement_unlocked": "Achievement: ", "settings": "Settings", "export_language": "Export Language", "sound": "Sound", "sound_enabled": "Sound Enabled", "sound_theme": "Sound Theme", "menu_click_label": "Menu Click", "click_label": "Click", "click_back_label": "Back Click", "hit_label": "Hit", "gameover_label": "Game Over", "menu_music_label": "Menu Music", "ingame_music_label": "In-Game Music", "on": "ON", "off": "OFF",
        "choose_language": "Choose your Language", "color_scheme": "Color Scheme", "lang_continue": "Continue",
        "rarity_label": "Rarity", "rarity_normal": "Normal", "rarity_rare": "Rare", "rarity_epic": "Epic",
        "achievements": "Achievements",
        "profiles": "Profiles", "add_profile": "Add Profile", "who_is_playing": "Who's playing?",
        "enter_name": "What's your name?", "confirm": "Confirm", "delete_profile": "Delete",
        "name_taken": "Name already taken!", "name_empty": "Please enter a name!", "name_filtered": "Name not allowed!",
        "auto_start_profile": "Auto-Start",
        "auto_start_hint": "Checkbox = always start with this profile.",
        "choose_difficulty_title": "Choose Difficulty", "selected_badge": "Selected", "press_a_key": "Press a key...",
        "no_coins_badge": "No Coins", "pts_per_coin": "{0} Pts = 1 Coin",
        "scroll_back_hint": "↑/↓: Scroll   ESC/M: Back", "future_version_notice": "Coming in a future version.",
        "achievements_disabled_notice": "Achievements are currently disabled.",
        "enter_click_close_hint": "ENTER / Click: Close", "enter_click_esc_close_hint": "ENTER/Click/ESC/M: Close",
        "enter_login_menu_hint": "ENTER: Login  |  ESC/M: Menu", "enter_confirm_menu_hint": "ENTER: Confirm  |  ESC/M: Menu",
        "esc_menu_hint": "ESC: Menu", "item_acquired": "{0} acquired!",
        "vs_mode_label": "VS", "coop_mode_label": "Co-op",
        "profile_hub_title": "Profile", "change_cosmetics": "Change Cosmetics", "switch_profile": "Switch Profile",
        "gui_size_label": "GUI Size:", "recommended_label": "Recommended", "background_label": "Background:",
        "bg_stars_toggle": "Stars", "bg_rings_toggle": "Rings", "bg_animated_toggle": "Animated", "bg_mono_toggle": "Mono", "bg_floor_toggle": "Floor",
        "controls_menu_btn": "Controls", "reset_defaults": "Reset to Default",
        "controls_title": "Customize Controls", "controls_hint_legend": "Click a key to remove  ·  [+] to add  ·  ⚠ = key also bound elsewhere",
        "controls_scroll_hint": "↑/↓ or mouse wheel: Scroll",
        "controls_tab_standard": "Standard", "controls_tab_2p": "2-Player", "controls_tab_menu": "Menu",
    },
    "Русский": {
        "ice_trail": "ледяной след",
        "shadow_trail": "тень след",
        "sparkle_trail": "искрящийся след",
        "toxic_trail": "токсичный след",
        "start": "Старт", "language": "Язык", "difficulty": "Сложность",
        "game_over": "Конец игры!", "retry": "Повтор",
        "score": "Очки", "lives": "Жизни", "highscore": "Рекорд", "stats": "Статистика",
        "back": "Назад", "stat_highscores": "Рекорды", "stat_achievements": "Достижения", "stat_total": "Всего", "stat_modded": "Modded",
        "no_achievements": "Нет достижений.", "total_time": "Общее время",
        "rounds_played": "Раундов сыграно", "1ups_collected": "1-ап собрано", "record": "Рекорд", "current": "Текущий",
        "table_difficulty": "Сложность", "table_highscore": "Рекорд", "table_session": "Сессия",
        "online_leaderboard_btn": "Онлайн-рейтинг", "search": "Поиск", "no_entries_yet": "Пока нет записей",
        "player_search_title": "Поиск игрока", "player_not_found": "Игрок не найден", "powers_used_label": "Использованные способности",
        "owned_powers_label": "Разблокированные способности", "owned_cosmetics_label": "Разблокированная косметика",
        "diff_Leicht": "Легко", "diff_Mittel": "Средне", "diff_Schwer": "Сложно", "diff_Hardcore": "Хардкор",
        "diff_Unmöglich": "Невозможно", "diff_Albtraum": "Кошмар", "diff_Hölle": "Ад", "diff_Dynamisch": "Динамический",
        "redeem_key_btn": "Активировать ключ", "rename_btn": "Изменить имя",
        "redeem_key_success": "Ключ успешно активирован!", "rename_success": "Имя изменено!",
        "key_invalid_msg": "Неверный ключ", "key_revoked_msg": "Этот ключ заблокирован",
        "key_shared_msg": "Ключ был передан другому - обе активации заблокированы",
        "unknown_player_msg": "Сначала сыграйте раунд в стандартном режиме", "play_time_label": "Время игры",
        "generic_error_msg": "Что-то пошло не так - сервер недоступен или ждёт обновления",
        "redeem_code_btn": "Активировать код", "redeem_code_success": "Код активирован!",
        "code_invalid_msg": "Неверный код", "code_already_redeemed_msg": "Код уже использован",
        "paste_btn": "Вставить", "add_friend_btn": "+ Друг", "remove_friend_btn": "- Друг",
        "friends_hub_title": "Друзья", "friends_tab_roster": "Мои друзья", "friends_tab_requests": "Заявки",
        "no_requests_yet": "Нет заявок", "accept_btn": "Принять", "decline_btn": "Отклонить",
        "message_btn": "Сообщение", "friend_request_sent_msg": "Заявка отправлена!", "already_friends_msg": "Вы уже друзья",
        "clan_hub_title": "Клан", "create_clan_btn": "Создать клан", "join_clan_btn": "Вступить в клан",
        "leave_clan_btn": "Покинуть", "kick_btn": "Кикнуть", "clan_tag_label": "Тег", "clan_name_label": "Название",
        "clan_not_in_one": "Вы не состоите в клане", "clan_members_label": "Участники", "clan_owner_label": "Лидер",
        "clan_search_title": "Поиск клана", "clan_not_found": "Клан не найден",
        "clan_fields_required_msg": "Введите тег и название", "clan_already_in_one_msg": "Вы уже состоите в клане",
        "clan_tag_taken_msg": "Тег уже занят", "clan_name_taken_msg": "Название уже занято", "clan_full_msg": "Клан заполнен",
        "chat_title": "Чат", "send_btn": "Отправить", "no_messages_yet": "Пока нет сообщений",
        "chat_message_too_long_msg": "Сообщение слишком длинное", "chat_message_filtered_msg": "Сообщение не разрешено",
        "chat_not_friends_msg": "Вы не друзья", "chat_not_clan_member_msg": "Вы не состоите в этом клане",
        "chat_rate_limited_msg": "Пожалуйста, подождите",
        "friends_tab": "Друзья", "all_tab_lb": "Все", "no_friends_yet": "Пока нет друзей",
        "total_jumps": "Прыжков всего", "total_deaths": "Поражений", "avg_score": "Ø очки", "total_coins_earned": "Всего монет заработано",
        "shop": "Магазин", "loadout": "Инвентарь", "coins": "Монеты",
        "power_roll": "Power Roll",
        "rollen": "Роллы",
        "coming_soon": "Скоро!",
        "admin": "Админ", "enter_password": "Пароль админа:", "wrong_password": "Неверный пароль",
        "add_coins": "Добавить монеты", "set_coins": "Задать монеты", "amount": "Количество",
        "logout": "Выйти", "give_power": "Выдать Power",
        "spin": "Крутить (50)", "spin10": "Крутить x10 (500)", "results": "Награды",
        "owned_tab": "Есть", "equipped_tab": "Надето", "all_tab": "Все", "no_owned": "Нет Powers.", "more": "Больше", "help": "Помощь",
        "classic_mode": "JumpNDodge Classic", "esc_back": "ESC: Назад",
        "game_subtitle": "Динамичная прыжковая игра", "controls_hint": "ENTER: Старт  |  W/S: Язык/Сложность  |  ESC: Выход",
        "back_to_main": "ESC/M: Назад в главное меню", "click_equip": "Клик: Надеть (Авто-замена при заполнении)  |  ESC/M: Меню",
        "locked": "заблокировано", "equipped": "надето", "passive": "пассивно", "auto_swap": "Авто-замена", "new_power": "Новое",
        "duplicate": "Дубликат", "extra_slot_unlocked": "Дополнительный слот разблокирован! (пассивно)", "power_added": "Сила добавлена!",
        "power_already_owned": "Сила уже есть", "protected": "Защищен!", "double_jump": "Двойной прыжок!",
        "slow_motion": "Замедление!", "time_freeze": "Заморозка времени!", "ghost_mode": "Режим призрака!", "nuke": "ЯДЕРКА!",
        "1up_collected": "1-ап собрано", "1up": "1-ап!", "insufficient_coins": "Недостаточно монет!",
        "roll_x10_complete": "Крутка x10 завершена", "roll_x10_coins": "Крутка x10: +{0} монет обратно",
        "description": "Описание", "not_owned": "Не владеете",
        "daily_reward": "Ежедневная награда", "daily_reward_claimed": "Уже получено",
        "daily_reward_no_internet": "Нет интернета", "daily_reward_available": "Доступно",
        "last_reward": "Последняя награда", "reset_time": "Сброс: 00:00",
        "dark_mode": "Тёмный режим", "light_mode": "Светлый режим",
        "cosmetic_inventory": "Косметический инвентарь",
        "cosmetic_shop": "Косметический магазин",
        "trails_tab": "Следы",
        "player_cosmetics_tab": "Косметика игрока",
        "headwear_tab": "Головные уборы",
        "face_tab": "Черты лица",
        "eyes_tab": "Глаза",
        "nose_tab": "Нос",
        "mouth_tab": "Рот",
        "tools_tab": "Инструменты",
        "tools_coming_soon": "Скоро!",
        "sale_view_title": "Сегодняшняя акция", "sale_next_in": "Следующая акция через:", "sale_no_items": "Сегодня нет акций.",
        "fire_trail": "огненный след",
        "yellow_crown": "желтая корона",
        "buy": "Купить",
        "owned": "Приобретено",
        "daily_rewards": "Ежедневные награды", "all_statistics": "Вся статистика", "difficulties": "Сложности",
        "loading": "Загрузка...", "claim_reward": "Получить награду", "next_reset_in": "Следующий сброс через:", "achievement_unlocked": "Достижение: ", "settings": "Настройки", "export_language": "Экспорт языка", "sound": "Звук", "sound_enabled": "Звук включен", "sound_theme": "Тема звука", "menu_click_label": "Клик меню", "click_label": "Клик", "click_back_label": "Клик назад", "hit_label": "Удар", "gameover_label": "Game Over", "menu_music_label": "Музыка меню", "ingame_music_label": "Игровая музыка", "on": "ВКЛ", "off": "ВЫКЛ",
        "choose_language": "Выберите язык", "color_scheme": "Цветовая схема", "lang_continue": "Далее",
        "rarity_label": "Редкость", "rarity_normal": "Обычный", "rarity_rare": "Редкий", "rarity_epic": "Эпический",
        "achievements": "Достижения",
        "profiles": "Профили", "add_profile": "Добавить профиль", "who_is_playing": "Кто играет?",
        "enter_name": "Как тебя зовут?", "confirm": "Подтвердить", "delete_profile": "Удалить",
        "name_taken": "Имя уже занято!", "name_empty": "Введите имя!", "name_filtered": "Имя не разрешено!",
        "auto_start_profile": "Автозапуск",
        "auto_start_hint": "Флажок = всегда начинать с этим профилем.",
        "choose_difficulty_title": "Выбери сложность", "selected_badge": "Выбрано", "press_a_key": "Нажми клавишу...",
        "no_coins_badge": "Без монет", "pts_per_coin": "{0} очк. = 1 монета",
        "scroll_back_hint": "↑/↓: Прокрутка   ESC/M: Назад", "future_version_notice": "Появится в будущей версии.",
        "achievements_disabled_notice": "Достижения сейчас отключены.",
        "enter_click_close_hint": "ENTER / Клик: Закрыть", "enter_click_esc_close_hint": "ENTER/Клик/ESC/M: Закрыть",
        "enter_login_menu_hint": "ENTER: Вход  |  ESC/M: Меню", "enter_confirm_menu_hint": "ENTER: Подтвердить  |  ESC/M: Меню",
        "esc_menu_hint": "ESC: Меню", "item_acquired": "{0} получено!",
        "vs_mode_label": "VS", "coop_mode_label": "Кооп",
        "profile_hub_title": "Профиль", "change_cosmetics": "Изменить косметику", "switch_profile": "Сменить профиль",
        "gui_size_label": "Размер интерфейса:", "recommended_label": "Рекомендовано", "background_label": "Фон:",
        "bg_stars_toggle": "Звёзды", "bg_rings_toggle": "Кольца", "bg_animated_toggle": "Анимировано", "bg_mono_toggle": "Моно", "bg_floor_toggle": "Пол",
        "controls_menu_btn": "Управление", "reset_defaults": "Сбросить на стандартные",
        "controls_title": "Настроить управление", "controls_hint_legend": "Нажми клавишу, чтобы удалить  ·  [+] чтобы добавить  ·  ⚠ = клавиша занята также в другом месте",
        "controls_scroll_hint": "↑/↓ или колесо мыши: Прокрутка",
        "controls_tab_standard": "Стандарт", "controls_tab_2p": "2 игрока", "controls_tab_menu": "Меню",
    },
    "Türkçe": {
        "ice_trail": "Buz İz",
        "shadow_trail": "Gölge İz",
        "sparkle_trail": "Parıltı İz",
        "toxic_trail": "Toksik İz",
        "start": "Başlat", "language": "Dil", "difficulty": "Zorluk",
        "game_over": "Oyun Bitti!", "retry": "Tekrar",
        "score": "Skor", "lives": "Can", "highscore": "Rekor", "stats": "İstatistik",
        "back": "Geri", "stat_highscores": "Rekorlar", "stat_achievements": "Başarılar", "stat_total": "Toplam", "stat_modded": "Modded",
        "no_achievements": "Henüz başarı yok.", "total_time": "Toplam Süre",
        "rounds_played": "Oynanan Turlar", "1ups_collected": "Toplanan 1-UP", "record": "Rekor", "current": "Şu an",
        "table_difficulty": "Zorluk", "table_highscore": "Rekor", "table_session": "Oturum",
        "online_leaderboard_btn": "Çevrimiçi Sıralama", "search": "Ara", "no_entries_yet": "Henüz kayıt yok",
        "player_search_title": "Oyuncu Ara", "player_not_found": "Oyuncu bulunamadı", "powers_used_label": "Kullanılan güçler",
        "owned_powers_label": "Açılan Güçler", "owned_cosmetics_label": "Açılan Kozmetikler",
        "diff_Leicht": "Kolay", "diff_Mittel": "Orta", "diff_Schwer": "Zor", "diff_Hardcore": "Hardcore",
        "diff_Unmöglich": "İmkansız", "diff_Albtraum": "Kâbus", "diff_Hölle": "Cehennem", "diff_Dynamisch": "Dinamik",
        "redeem_key_btn": "Doğrulama Anahtarı Kullan", "rename_btn": "İsmi Değiştir",
        "redeem_key_success": "Anahtar başarıyla kullanıldı!", "rename_success": "İsim değiştirildi!",
        "key_invalid_msg": "Geçersiz anahtar", "key_revoked_msg": "Bu anahtar iptal edildi",
        "key_shared_msg": "Anahtar başkasıyla paylaşıldı - her iki kullanım da iptal edildi",
        "unknown_player_msg": "Önce bir Standart tur oyna", "play_time_label": "Oynama süresi",
        "generic_error_msg": "Bir şeyler ters gitti - sunucuya ulaşılamıyor veya güncelleme bekleniyor",
        "redeem_code_btn": "Kod Kullan", "redeem_code_success": "Kod kullanıldı!",
        "code_invalid_msg": "Geçersiz kod", "code_already_redeemed_msg": "Kod zaten kullanıldı",
        "paste_btn": "Yapıştır", "add_friend_btn": "+ Arkadaş", "remove_friend_btn": "- Arkadaş",
        "friends_hub_title": "Arkadaşlar", "friends_tab_roster": "Arkadaşlarım", "friends_tab_requests": "İstekler",
        "no_requests_yet": "İstek yok", "accept_btn": "Kabul Et", "decline_btn": "Reddet",
        "message_btn": "Mesaj", "friend_request_sent_msg": "İstek gönderildi!", "already_friends_msg": "Zaten arkadaşsınız",
        "clan_hub_title": "Klan", "create_clan_btn": "Klan Kur", "join_clan_btn": "Klana Katıl",
        "leave_clan_btn": "Ayrıl", "kick_btn": "At", "clan_tag_label": "Etiket", "clan_name_label": "İsim",
        "clan_not_in_one": "Bir klanda değilsin", "clan_members_label": "Üyeler", "clan_owner_label": "Lider",
        "clan_search_title": "Klan Arama", "clan_not_found": "Klan bulunamadı",
        "clan_fields_required_msg": "Lütfen etiket ve isim gir", "clan_already_in_one_msg": "Zaten bir klandasın",
        "clan_tag_taken_msg": "Etiket zaten alınmış", "clan_name_taken_msg": "İsim zaten alınmış", "clan_full_msg": "Klan dolu",
        "chat_title": "Sohbet", "send_btn": "Gönder", "no_messages_yet": "Henüz mesaj yok",
        "chat_message_too_long_msg": "Mesaj çok uzun", "chat_message_filtered_msg": "Mesaja izin verilmiyor",
        "chat_not_friends_msg": "Arkadaş değilsiniz", "chat_not_clan_member_msg": "Bu klanda değilsin",
        "chat_rate_limited_msg": "Lütfen biraz bekle",
        "friends_tab": "Arkadaşlar", "all_tab_lb": "Tümü", "no_friends_yet": "Henüz arkadaş eklenmedi",
        "total_jumps": "Toplam Zıplama", "total_deaths": "Kaybedilen Oyunlar", "avg_score": "Ø Skor", "total_coins_earned": "Toplam Kazanılan Coin",
        "shop": "Mağaza", "loadout": "Envanter", "coins": "Paralar",
        "power_roll": "Power Roll",
        "rollen": "Rollar",
        "coming_soon": "Yakında!",
        "admin": "Admin", "enter_password": "Admin şifresi:", "wrong_password": "Yanlış şifre",
        "add_coins": "Para ekle", "set_coins": "Para ayarla", "amount": "Miktar",
        "logout": "Çıkış", "give_power": "Power ver",
        "spin": "Çevir (50)", "spin10": "10x Çevir (500)", "results": "Ödüller",
        "owned_tab": "Sahip", "equipped_tab": "Takılı", "all_tab": "Tümü", "no_owned": "Powers yok.", "more": "Daha Fazla", "help": "Yardım",
        "classic_mode": "JumpNDodge Classic", "esc_back": "ESC: Geri",
        "game_subtitle": "Aksiyon dolu zıplama oyunu", "controls_hint": "ENTER: Başlat  |  W/S: Dil/Zorluk  |  ESC: Çıkış",
        "back_to_main": "ESC/M: Ana menüye dön", "click_equip": "Tıkla: Tak (Dolu iken Otomatik Değiştir)  |  ESC/M: Menü",
        "locked": "kilitli", "equipped": "takılı", "passive": "pasif", "auto_swap": "Otomatik Değiştir", "new_power": "Yeni",
        "duplicate": "Kopya", "extra_slot_unlocked": "Ekstra Yuva açıldı! (pasif)", "power_added": "Güç eklendi!",
        "power_already_owned": "Güç zaten var", "protected": "Korundu!", "double_jump": "Çift Zıplama!",
        "slow_motion": "Yavaş Hareket!", "time_freeze": "Zaman Dondurma!", "ghost_mode": "Hayalet Modu!", "nuke": "NÜKLEER!",
        "1up_collected": "1-UP toplandı", "1up": "1-UP!", "insufficient_coins": "Yetersiz para!",
        "roll_x10_complete": "10x Çevirme tamamlandı", "roll_x10_coins": "10x Çevirme: +{0} para geri",
        "description": "Açıklama", "not_owned": "Sahip değil",
        "daily_reward": "Günlük Ödül", "daily_reward_claimed": "Zaten alındı",
        "daily_reward_no_internet": "İnternet yok", "daily_reward_available": "Mevcut",
        "last_reward": "Son Ödül", "reset_time": "Sıfırlama: 00:00",
        "dark_mode": "Karanlık Mod", "light_mode": "Açık Mod",
        "cosmetic_inventory": "Kozmetik Envanter",
        "cosmetic_shop": "Kozmetik Mağaza",
        "trails_tab": "İzler",
        "player_cosmetics_tab": "Oyuncu Kozmetikleri",
        "headwear_tab": "Baş Giyimi",
        "face_tab": "Yüz Özellikleri",
        "eyes_tab": "Gözler",
        "nose_tab": "Burun",
        "mouth_tab": "Ağız",
        "tools_tab": "Araçlar",
        "tools_coming_soon": "Yakında!",
        "sale_view_title": "Bugünün İndirimi", "sale_next_in": "Sonraki indirim:", "sale_no_items": "Bugün indirim yok.",
        "fire_trail": "ateşli iz",
        "yellow_crown": "Sarı Taç",
        "buy": "Satın Al",
        "owned": "Sahip Olunan",
        "daily_rewards": "Günlük Ödüller", "all_statistics": "Tüm İstatistikler", "difficulties": "Zorluklar",
        "loading": "Yükleniyor...", "claim_reward": "Ödülü Al", "next_reset_in": "Sonraki sıfırlama:", "achievement_unlocked": "Başarı: ", "settings": "Ayarlar", "export_language": "Dili Dışa Aktar", "sound": "Ses", "sound_enabled": "Ses Etkin", "sound_theme": "Ses Teması", "menu_click_label": "Menü Tıklaması", "click_label": "Tıklama", "click_back_label": "Geri Tıklaması", "hit_label": "Vuruş", "gameover_label": "Game Over", "menu_music_label": "Menü Müziği", "ingame_music_label": "Oyun Müziği", "on": "AÇIK", "off": "KAPALI",
        "choose_language": "Dilinizi Seçin", "color_scheme": "Renk Şeması", "lang_continue": "Devam",
        "rarity_label": "Nadirlik", "rarity_normal": "Normal", "rarity_rare": "Nadir", "rarity_epic": "Destansı",
        "achievements": "Başarılar",
        "profiles": "Profiller", "add_profile": "Profil Ekle", "who_is_playing": "Kim oynuyor?",
        "enter_name": "Adın ne?", "confirm": "Onayla", "delete_profile": "Sil",
        "name_taken": "Bu isim alındı!", "name_empty": "Lütfen bir isim girin!", "name_filtered": "Bu isme izin verilmiyor!",
        "auto_start_profile": "Otomatik Başlat",
        "auto_start_hint": "Kutu = her zaman bu profille başla.",
        "choose_difficulty_title": "Zorluk Seç", "selected_badge": "Seçildi", "press_a_key": "Bir tuşa bas...",
        "no_coins_badge": "Coin Yok", "pts_per_coin": "{0} Puan = 1 Coin",
        "scroll_back_hint": "↑/↓: Kaydır   ESC/M: Geri", "future_version_notice": "Gelecek bir sürümde geliyor.",
        "achievements_disabled_notice": "Başarılar şu anda devre dışı.",
        "enter_click_close_hint": "ENTER / Tıkla: Kapat", "enter_click_esc_close_hint": "ENTER/Tıkla/ESC/M: Kapat",
        "enter_login_menu_hint": "ENTER: Giriş  |  ESC/M: Menü", "enter_confirm_menu_hint": "ENTER: Onayla  |  ESC/M: Menü",
        "esc_menu_hint": "ESC: Menü", "item_acquired": "{0} kazanıldı!",
        "vs_mode_label": "VS", "coop_mode_label": "Koop",
        "profile_hub_title": "Profil", "change_cosmetics": "Kozmetikleri Değiştir", "switch_profile": "Profili Değiştir",
        "gui_size_label": "Arayüz Boyutu:", "recommended_label": "Önerilen", "background_label": "Arka Plan:",
        "bg_stars_toggle": "Yıldızlar", "bg_rings_toggle": "Halkalar", "bg_animated_toggle": "Animasyonlu", "bg_mono_toggle": "Mono", "bg_floor_toggle": "Zemin",
        "controls_menu_btn": "Kontroller", "reset_defaults": "Varsayılana Sıfırla",
        "controls_title": "Kontrolleri Özelleştir", "controls_hint_legend": "Kaldırmak için bir tuşa tıkla  ·  Eklemek için [+]  ·  ⚠ = tuş başka yerde de kullanılıyor",
        "controls_scroll_hint": "↑/↓ veya fare tekerleği: Kaydır",
        "controls_tab_standard": "Standart", "controls_tab_2p": "2 Oyuncu", "controls_tab_menu": "Menü",
    },
    "Schwiizerdütsch": {
        "ice_trail": "Iis Spur",
        "shadow_trail": "Schatte Spur",
        "sparkle_trail": "Funkle Spur",
        "toxic_trail": "Gift Spur",
        "start": "Start", "language": "Sproch", "difficulty": "Schwierigkeit",
        "game_over": "Fertig!", "retry": "Nomol",
        "score": "Punkt", "lives": "Läbe", "highscore": "Rekord", "stats": "Statistik",
        "back": "Zrugg", "stat_highscores": "Rekord", "stat_achievements": "Erfolg", "stat_total": "Total", "stat_modded": "Modded",
        "no_achievements": "Kei Erfolg bis jetzt.", "total_time": "Ziit gspiilt",
        "rounds_played": "Runde gspiilt", "1ups_collected": "1-UP gsammlt", "record": "Rekord", "current": "Jetzt",
        "table_difficulty": "Schwierigkeit", "table_highscore": "Rekord", "table_session": "Sitzung",
        "online_leaderboard_btn": "Online-Rangliste", "search": "Suche", "no_entries_yet": "Noch keine Einträge",
        "player_search_title": "Spielersuche", "player_not_found": "Spieler nicht gefunden", "powers_used_label": "Verwendete Powers",
        "owned_powers_label": "Freigeschaltete Powers", "owned_cosmetics_label": "Freigeschaltete Cosmetics",
        "diff_Leicht": "Leicht", "diff_Mittel": "Mittel", "diff_Schwer": "Schwer", "diff_Hardcore": "Hardcore",
        "diff_Unmöglich": "Unmöglich", "diff_Albtraum": "Albtraum", "diff_Hölle": "Hölle", "diff_Dynamisch": "Dynamisch",
        "redeem_key_btn": "Verified-Key einlösen", "rename_btn": "Namen ändern",
        "redeem_key_success": "Key erfolgreich eingelöst!", "rename_success": "Name geändert!",
        "key_invalid_msg": "Ungültiger Key", "key_revoked_msg": "Dieser Key wurde gesperrt",
        "key_shared_msg": "Key wurde von jemandem geteilt - beide Nutzungen gesperrt",
        "unknown_player_msg": "Bitte zuerst eine Standard-Runde spielen", "play_time_label": "Spielzeit",
        "generic_error_msg": "Etwas ist schiefgelaufen - Server nicht erreichbar oder Update ausstehend",
        "redeem_code_btn": "Code einlösen", "redeem_code_success": "Code eingelöst!",
        "code_invalid_msg": "Ungültiger Code", "code_already_redeemed_msg": "Code bereits eingelöst",
        "paste_btn": "Einfügen", "add_friend_btn": "+ Freund", "remove_friend_btn": "- Freund",
        "friends_hub_title": "Fründe", "friends_tab_roster": "Mini Fründe", "friends_tab_requests": "Aafrage",
        "no_requests_yet": "Kei Aafrage", "accept_btn": "Aaneh", "decline_btn": "Ablehne",
        "message_btn": "Nachricht", "friend_request_sent_msg": "Aafrag gschickt!", "already_friends_msg": "Ihr sind scho befründet",
        "clan_hub_title": "Clan", "create_clan_btn": "Clan erstelle", "join_clan_btn": "Clan bitrete",
        "leave_clan_btn": "Verlah", "kick_btn": "Chick", "clan_tag_label": "Tag", "clan_name_label": "Name",
        "clan_not_in_one": "Du bisch i keim Clan", "clan_members_label": "Mitglieder", "clan_owner_label": "Aafüehrer",
        "clan_search_title": "Clan-Suech", "clan_not_found": "Clan nid gfunde",
        "clan_fields_required_msg": "Bitte Tag und Name igäh", "clan_already_in_one_msg": "Du bisch scho i eim Clan",
        "clan_tag_taken_msg": "Tag scho vergäbe", "clan_name_taken_msg": "Name scho vergäbe", "clan_full_msg": "Clan isch voll",
        "chat_title": "Chat", "send_btn": "Schicke", "no_messages_yet": "No kei Nachrichte",
        "chat_message_too_long_msg": "Nachricht z'lang", "chat_message_filtered_msg": "Nachricht nid erlaubt",
        "chat_not_friends_msg": "Ihr sind nid befründet", "chat_not_clan_member_msg": "Du bisch nid i däm Clan",
        "chat_rate_limited_msg": "Bitte churz warte",
        "friends_tab": "Freunde", "all_tab_lb": "Alle", "no_friends_yet": "Noch keine Freunde markiert",
        "total_jumps": "Sprüng total", "total_deaths": "Game Overs", "avg_score": "Ø Score", "total_coins_earned": "Coins total verdiänt",
        "shop": "Lade", "loadout": "Inventar", "coins": "Münze",
        "power_roll": "Power Roll",
        "rollen": "Rollen",
        "coming_soon": "Baldi!",
        "admin": "Admin", "enter_password": "Admin-Passwort:", "wrong_password": "Falschs Passwort",
        "add_coins": "Münze zuefüege", "set_coins": "Münze setze", "amount": "Mängi",
        "logout": "Uslogge", "give_power": "Power gäh",
        "spin": "Drehe (50)", "spin10": "Roll x10 (500)", "results": "Belohnige",
        "owned_tab": "Hesch", "equipped_tab": "Aaglegt", "all_tab": "Alli", "no_owned": "Kei Powers.", "more": "Mehr", "help": "Hilf",
        "classic_mode": "JumpNDodge Classic", "esc_back": "ESC: Zrugg",
        "game_subtitle": "Es actionriichs Springspiil", "controls_hint": "ENTER: Start  |  W/S: Sproch/Schwierigkeit  |  ESC: Usstieg",
        "back_to_main": "ESC/M: Zrugg zum Hauptmenü", "click_equip": "Klick: Aalegge (Auto-Tuusche wenn voll)  |  ESC/M: Menü",
        "locked": "gschperrt", "equipped": "aaglegt", "passive": "passiv", "auto_swap": "Auto-Tuusche", "new_power": "Neu",
        "duplicate": "Duplikat", "extra_slot_unlocked": "Extra Slot freigschaltet! (passiv)", "power_added": "Power dezuegfüegt!",
        "power_already_owned": "Power scho bessesse", "protected": "Gschützt!", "double_jump": "Dopplet Springe!",
        "slow_motion": "Langsami Bewegig!", "time_freeze": "Ziit friere!", "ghost_mode": "Gäist-Modus!", "nuke": "ATOMBOMBE!",
        "1up_collected": "1-UP gsammlt", "1up": "1-UP!", "insufficient_coins": "Z wenig Münze!",
        "roll_x10_complete": "Roll x10 fertig", "roll_x10_coins": "Roll x10: +{0} Münze zrugg",
        "description": "Beschriibig", "not_owned": "Nöd bessesse",
        "daily_reward": "Tägliche Belohnig", "daily_reward_claimed": "Schu erhalte",
        "daily_reward_no_internet": "Kei Internet", "daily_reward_available": "Verfüegbar",
        "last_reward": "Letschti Belohnig", "reset_time": "Reset: 00:00",
        "dark_mode": "Dunkle Modus", "light_mode": "Light Modus",
        "cosmetic_inventory": "Kosmetik Inventar",
        "cosmetic_shop": "Kosmetik Shop",
        "trails_tab": "Spure",
        "player_cosmetics_tab": "Spieler Kosmetik",
        "headwear_tab": "Kopfbedeckige",
        "face_tab": "Gsichtsmerkmal",
        "eyes_tab": "Ouge",
        "nose_tab": "Nase",
        "mouth_tab": "Mund",
        "tools_tab": "Tools",
        "tools_coming_soon": "Bald verfüegbar!",
        "sale_view_title": "Hütigs Aagebot", "sale_next_in": "Nächschts Aagebot in:", "sale_no_items": "Hüt kes Aagebot verfüegbar.",
        "fire_trail": "Feierig Spur",
        "yellow_crown": "Gäl Krone",
        "buy": "Kaufe",
        "owned": "Ghesch",
        "daily_rewards": "Tägliichi Belohnige", "all_statistics": "Alli Statistike", "difficulties": "Schwierigkeite",
        "loading": "Ladet...", "claim_reward": "Belohnig abhole", "next_reset_in": "Nächschter Reset in:", "achievement_unlocked": "Erfolg: ", "settings": "Iistellige", "export_language": "Sproch exportiere", "sound": "Sound", "sound_enabled": "Sound aktiviert", "sound_theme": "Sound-Thema", "menu_click_label": "Menü-Klick", "click_label": "Klick", "click_back_label": "Zrugg-Klick", "hit_label": "Treffer", "gameover_label": "Game Over", "menu_music_label": "Menü-Musik", "ingame_music_label": "Spiil-Musik", "on": "A", "off": "US",
        "choose_language": "Wähl dini Sproch", "color_scheme": "Farbschema", "lang_continue": "Witer",
        "rarity_label": "Seltenheit", "rarity_normal": "Normal", "rarity_rare": "Selte", "rarity_epic": "Episch",
        "achievements": "Ervolg",
        "profiles": "Profile", "add_profile": "Profil hinzuefieje", "who_is_playing": "Wer spielt?",
        "enter_name": "Wie heischsch du?", "confirm": "Bestätige", "delete_profile": "Lösche",
        "name_taken": "Name scho vergäbe!", "name_empty": "Bitte en Name igäh!", "name_filtered": "Name nid erlaubt!",
        "auto_start_profile": "Auto-Start",
        "auto_start_hint": "Häckli = immer mit dem Profil starte.",
        "choose_difficulty_title": "Schwierigkeit uswähle", "selected_badge": "Uswählt", "press_a_key": "Tascht drücke...",
        "no_coins_badge": "Kei Coins", "pts_per_coin": "{0} Punkt = 1 Coin",
        "scroll_back_hint": "↑/↓: Scroll   ESC/M: Zrugg", "future_version_notice": "Chunt i re zuekünftige Version.",
        "achievements_disabled_notice": "Erfolg sind momentan deaktiviert.",
        "enter_click_close_hint": "ENTER / Klick: Zuemache", "enter_click_esc_close_hint": "ENTER/Klick/ESC/M: Zuemache",
        "enter_login_menu_hint": "ENTER: Login  |  ESC/M: Menü", "enter_confirm_menu_hint": "ENTER: Bestätige  |  ESC/M: Menü",
        "esc_menu_hint": "ESC: Menü", "item_acquired": "{0} übercho!",
        "vs_mode_label": "VS", "coop_mode_label": "Koop",
        "profile_hub_title": "Profil", "change_cosmetics": "Kosmetik ändere", "switch_profile": "Profil wächsle",
        "gui_size_label": "GUI Grössi:", "recommended_label": "Empfohle", "background_label": "Hindergrund:",
        "bg_stars_toggle": "Stärne", "bg_rings_toggle": "Ringe", "bg_animated_toggle": "Animiert", "bg_mono_toggle": "Mono", "bg_floor_toggle": "Bode",
        "controls_menu_btn": "Steurig", "reset_defaults": "Standard wiederherstelle",
        "controls_title": "Steurig aapasse", "controls_hint_legend": "Klick e Tascht zum Entferne  ·  [+] zum Dezuefüege  ·  ⚠ = Tascht au anderswo belegt",
        "controls_scroll_hint": "↑/↓ oder Muusrad: Scrolle",
        "controls_tab_standard": "Standard", "controls_tab_2p": "2-Spieler", "controls_tab_menu": "Menü",
    },
}

# =============================
# Custom Languages Loader (Custom-1/2/3, nur TXT)
# =============================
def load_custom_languages():
    """
    Lädt Custom-1.txt bis Custom-10.txt als komplett neue Sprachen.
    Unterstützt nur key=value Format (eine Zeile pro Eintrag).
    """
    for i in range(1, 11):
        fname_txt = f"Custom-{i}.txt"
        fpath_txt = os.path.join(BASE_DIR, fname_txt)

        if not os.path.exists(fpath_txt):
            continue

        langname = f"Custom Language {i}"
        try:
            lang_dict = {}
            with open(fpath_txt, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith("#"):
                        continue
                    if "=" in line:
                        key, val = line.split("=", 1)
                        val = val.strip()
                        # Remove surrounding quotes if present
                        if (val.startswith('"') and val.endswith('"')) or (val.startswith("'") and val.endswith("'")):
                            val = val[1:-1]
                        lang_dict[key.strip()] = val

            # Sprache als komplett eigenständige Sprache registrieren
            texts[langname] = lang_dict
            if langname not in languages:
                languages.append(langname)

            print(f"[INFO] {fname_txt} geladen -> {langname} ({len(lang_dict)} Einträge)")
        except Exception as e:
            try:
                print(f"[FEHLER] Konnte {fname_txt} nicht laden: {e}")
            except UnicodeEncodeError:
                print(f"[FEHLER] Konnte {fname_txt} nicht laden: Encoding error - check file encoding")

# Direkt beim Import/Start laden
try:
    load_custom_languages()
except Exception as e:
    print('[WARN] Fehler beim Laden von Custom-Sprachen:', e)

def export_current_language():
    """Export the current language to a Custom-x.txt file on Desktop."""
    current_lang = languages[language_index]
    lang_dict = texts.get(current_lang, {})

    # Collect all unique keys from all languages
    all_keys = set()
    for lang in languages:
        if lang in texts and isinstance(texts[lang], dict):
            all_keys.update(texts[lang].keys())

    # For each key, use current language value, fallback to English
    english = texts.get("English", {})
    exported = {}
    for key in all_keys:
        val = lang_dict.get(key, english.get(key, key))
        exported[key] = val

    # Find next available Custom-x.txt on Desktop (1-10, cycle back to 1 after 10)
    desktop_path = os.path.expanduser("~/Desktop")
    x = 1
    for i in range(1, 11):
        if not os.path.exists(os.path.join(desktop_path, f"Custom-{i}.txt")):
            x = i
            break
    else:
        x = 1  # All 1-10 exist, overwrite 1

    # Write to file on Desktop
    filename = os.path.join(desktop_path, f"Custom-{x}.txt")
    try:
        with open(filename, "w", encoding="utf-8") as f:
            for key in sorted(exported.keys()):
                val = exported[key]
                # Quote if contains spaces, tabs, or newlines
                if " " in val or "\t" in val or "\n" in val:
                    val = f'"{val}"'
                f.write(f"{key}={val}\n")
        global last_export_message
        last_export_message = f"Last export: Custom-{x}.txt on Desktop"
        push_overlay(f"Exported as Custom-{x}.txt on Desktop", (0, 200, 0))
        play_sound(click_sound)
    except Exception as e:
        push_overlay(f"Export failed: {e}", (200, 0, 0))
        play_sound(click_sound)


# =============================
# Difficulties
# =============================

difficulties = ["Leicht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Albtraum", "Hölle", "Dynamisch"]

# Mehrsprachige Schwierigkeitsgrade
difficulty_translations = {
    "Deutsch": ["Leicht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Albtraum", "Hölle", "Dynamisch"],
    "English": ["Easy", "Medium", "Hard", "Hardcore", "Impossible", "Nightmare", "Hell", "Dynamic"],
    "Русский": ["Легко", "Средне", "Сложно", "Хардкор", "Невозможно", "Кошмар", "Ад", "Динамично"],
    "Türkçe": ["Kolay", "Orta", "Zor", "Hardcore", "İmkansız", "Kabus", "Cehennem", "Dinamik"],
    "Schwiizerdütsch": ["Licht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Alptraum", "Höll", "Dynamisch"]
}

def get_difficulty_name(difficulty_key):
    """Gibt den Schwierigkeitsgrad in der aktuellen Sprache zurück"""
    current_lang = languages[language_index]
    if current_lang in difficulty_translations:
        try:
            index = difficulties.index(difficulty_key)
            return difficulty_translations[current_lang][index]
        except (ValueError, IndexError):
            return difficulty_key
    return difficulty_key
difficulty_speeds = {"Leicht": 6, "Mittel": 8, "Schwer": 11, "Hardcore": 15, "Unmöglich": 18, "Albtraum": 24, "Hölle": 32, "Dynamisch": 7}
difficulty_obstacle_intervals = {
    "Leicht": (1400, 2200), "Mittel": (1000, 1800), "Schwer": (600, 1300),
    "Hardcore": (400, 1000), "Unmöglich": (500, 1000), "Albtraum": (260, 500), "Hölle": (120, 300), "Dynamisch": (1400, 2400)
}

class ObstacleDelayBag:
    """Hindernis-Delays: einfacher gleichverteilter Zufallswert im Bereich (min_delay, max_delay)."""

    def next(self, min_delay, max_delay):
        return random.randint(min_delay, max_delay)

# Obstacle type weights per difficulty.
# Types: "normal" 50×50 ground | "small" 30×30 ground | "large" 75×75 ground
#        "floating" 50×50 mid-air (pass under) | "double" two normals in quick succession
difficulty_obstacle_types = {
    "Leicht":    {"normal": 100},
    "Mittel":    {"normal": 100},
    "Schwer":    {"normal": 100},
    "Hardcore":  {"normal": 100},
    "Unmöglich": {"normal": 100},
    "Albtraum":  {"normal": 100},
    "Hölle":     {"normal": 100},
    "Dynamisch": {},  # computed at runtime
}

class Obstacle(pygame.Rect):
    """pygame.Rect subclass that carries an obstacle type tag and optionale Richtung.

    dir_x/dir_y (-1/0/1) werden nur vom Free-Roam-Modifier genutzt, damit Hindernisse aus
    jeder Bildschirmkante kommen können. None = Standardverhalten (von rechts, -current_speed)."""
    def __init__(self, left, top, width, height, obs_type="normal", dir_x=None, dir_y=None):
        super().__init__(left, top, width, height)
        self.obs_type = obs_type
        self.dir_x = dir_x
        self.dir_y = dir_y

def pick_obstacle_type(difficulty, dynamic_speed=None):
    """Return a random obstacle type string based on difficulty (and current speed for Dynamic mode)."""
    if klassik_mode:
        return "normal"
    if difficulty == "Dynamisch" and dynamic_speed is not None:
        weights = {"normal": 100}
    else:
        weights = difficulty_obstacle_types.get(difficulty, {"normal": 100})
        if not weights:
            weights = {"normal": 100}
    types, probs = zip(*weights.items())
    return random.choices(types, weights=probs, k=1)[0]

def _rand_obs_size():
    return 50

def make_obstacle(spawn_x, obs_type):
    """Return a list of Obstacle objects for the given type."""
    if obs_type == "double":
        s1, s2 = _rand_obs_size(), _rand_obs_size()
        gap = random.randint(80, 120)
        return [
            Obstacle(spawn_x,            HEIGHT - 50 - s1, s1, s1, "normal"),
            Obstacle(spawn_x + s1 + gap, HEIGHT - 50 - s2, s2, s2, "normal"),
        ]
    else:
        s = _rand_obs_size()
        return [Obstacle(spawn_x, HEIGHT - 50 - s, s, s, "normal")]

def make_top_obstacle():
    """Return a single Obstacle that falls from the top of the screen."""
    rx = random.randint(60, WIDTH - 110)
    return Obstacle(rx, -55, 50, 50, "floating")

def make_free_roam_obstacle():
    """Hindernis für den Free-Roam-Modifier: kommt von einer zufälligen Position entlang
    einer zufälligen Bildschirmkante (nicht nur aus den 4 Ecken) und fliegt geradeaus,
    mit zufälliger diagonaler Schräge, zur gegenüberliegenden Seite."""
    s = _rand_obs_size()
    edge = random.choice(["left", "right", "top", "bottom"])
    skew = random.uniform(-0.8, 0.8)  # diagonale Schräge relativ zur Haupt-Richtung

    if edge in ("left", "right"):
        y = random.randint(0, HEIGHT - s)
        x = -s if edge == "left" else WIDTH
        dx = 1 if edge == "left" else -1
        dy = skew
    else:
        x = random.randint(0, WIDTH - s)
        y = -s if edge == "top" else HEIGHT
        dy = 1 if edge == "top" else -1
        dx = skew

    # Richtung normalisieren, damit Geschwindigkeit unabhängig von der Schräge konstant bleibt
    mag = (dx * dx + dy * dy) ** 0.5
    dx, dy = dx / mag, dy / mag

    return Obstacle(x, y, s, s, "normal", dir_x=dx, dir_y=dy)

# =============================
# Keybinding System
# =============================

_DEFAULT_KB = {
    "jump":   [pygame.K_SPACE, pygame.K_w, pygame.K_UP],
    "left":   [pygame.K_LEFT,  pygame.K_a],
    "right":  [pygame.K_RIGHT, pygame.K_d],
    "power1": [pygame.K_q],
    "power2": [pygame.K_e],
    "pause":  [pygame.K_ESCAPE],
    "p1_jump":  [pygame.K_SPACE, pygame.K_w],
    "p1_left":  [pygame.K_a],
    "p1_right": [pygame.K_d],
    "p2_jump":  [pygame.K_UP],
    "p2_left":  [pygame.K_LEFT],
    "p2_right": [pygame.K_RIGHT],
}

_STANDARD_ACTIONS = ["jump", "left", "right", "power1", "power2", "pause"]
_2P_ACTIONS = ["p1_jump", "p1_left", "p1_right", "p2_jump", "p2_left", "p2_right"]
_MENU_NAV_ACTIONS = ["nav_up", "nav_down", "nav_left", "nav_right", "nav_confirm", "nav_back"]

_ACTION_LABELS_TRANSLATIONS = {
    "Deutsch": {
        "jump": "Springen", "left": "Links", "right": "Rechts", "power1": "Power 1", "power2": "Power 2", "pause": "Pause",
        "p1_jump": "2P – Spieler 1: Springen", "p1_left": "2P – Spieler 1: Links", "p1_right": "2P – Spieler 1: Rechts",
        "p2_jump": "2P – Spieler 2: Springen", "p2_left": "2P – Spieler 2: Links", "p2_right": "2P – Spieler 2: Rechts",
        "nav_up": "Hoch", "nav_down": "Runter", "nav_left": "Links", "nav_right": "Rechts", "nav_confirm": "Bestätigen", "nav_back": "Zurück",
    },
    "English": {
        "jump": "Jump", "left": "Left", "right": "Right", "power1": "Power 1", "power2": "Power 2", "pause": "Pause",
        "p1_jump": "2P – Player 1: Jump", "p1_left": "2P – Player 1: Left", "p1_right": "2P – Player 1: Right",
        "p2_jump": "2P – Player 2: Jump", "p2_left": "2P – Player 2: Left", "p2_right": "2P – Player 2: Right",
        "nav_up": "Up", "nav_down": "Down", "nav_left": "Left", "nav_right": "Right", "nav_confirm": "Confirm", "nav_back": "Back",
    },
    "Русский": {
        "jump": "Прыжок", "left": "Влево", "right": "Вправо", "power1": "Power 1", "power2": "Power 2", "pause": "Пауза",
        "p1_jump": "2P – Игрок 1: Прыжок", "p1_left": "2P – Игрок 1: Влево", "p1_right": "2P – Игрок 1: Вправо",
        "p2_jump": "2P – Игрок 2: Прыжок", "p2_left": "2P – Игрок 2: Влево", "p2_right": "2P – Игрок 2: Вправо",
        "nav_up": "Вверх", "nav_down": "Вниз", "nav_left": "Влево", "nav_right": "Вправо", "nav_confirm": "Подтвердить", "nav_back": "Назад",
    },
    "Türkçe": {
        "jump": "Zıpla", "left": "Sol", "right": "Sağ", "power1": "Power 1", "power2": "Power 2", "pause": "Duraklat",
        "p1_jump": "2P – Oyuncu 1: Zıpla", "p1_left": "2P – Oyuncu 1: Sol", "p1_right": "2P – Oyuncu 1: Sağ",
        "p2_jump": "2P – Oyuncu 2: Zıpla", "p2_left": "2P – Oyuncu 2: Sol", "p2_right": "2P – Oyuncu 2: Sağ",
        "nav_up": "Yukarı", "nav_down": "Aşağı", "nav_left": "Sol", "nav_right": "Sağ", "nav_confirm": "Onayla", "nav_back": "Geri",
    },
    "Schwiizerdütsch": {
        "jump": "Springe", "left": "Links", "right": "Rechts", "power1": "Power 1", "power2": "Power 2", "pause": "Pause",
        "p1_jump": "2P – Spieler 1: Springe", "p1_left": "2P – Spieler 1: Links", "p1_right": "2P – Spieler 1: Rechts",
        "p2_jump": "2P – Spieler 2: Springe", "p2_left": "2P – Spieler 2: Links", "p2_right": "2P – Spieler 2: Rechts",
        "nav_up": "Uehi", "nav_down": "Aabe", "nav_left": "Links", "nav_right": "Rechts", "nav_confirm": "Bestätige", "nav_back": "Zrugg",
    },
}

def get_action_label(action):
    """Gibt die lokalisierte Beschriftung für eine (Re-)Bind-Aktion zurück (Steuerung-Screen)."""
    lang = languages[language_index]
    pack = _ACTION_LABELS_TRANSLATIONS.get(lang, _ACTION_LABELS_TRANSLATIONS["Deutsch"])
    return pack.get(action, _ACTION_LABELS_TRANSLATIONS["Deutsch"].get(action, action))

def get_keybindings():
    stored = settings.get("keybindings", {})
    result = {}
    for action, defaults in _DEFAULT_KB.items():
        result[action] = stored.get(action, list(defaults))
    return result

def save_keybindings(kb):
    settings["keybindings"] = kb
    save_section("settings", settings)

def key_display_name(k):
    _special = {
        pygame.K_SPACE: "Space", pygame.K_RETURN: "Enter",
        pygame.K_LEFT: "Links", pygame.K_RIGHT: "Rechts",
        pygame.K_UP: "Hoch",   pygame.K_DOWN: "Runter",
        pygame.K_LSHIFT: "LShift", pygame.K_RSHIFT: "RShift",
        pygame.K_LCTRL: "LCtrl",  pygame.K_RCTRL: "RCtrl",
        pygame.K_LALT: "LAlt",    pygame.K_RALT: "RAlt",
        pygame.K_BACKSPACE: "Backspace", pygame.K_TAB: "Tab",
        pygame.K_ESCAPE: "Escape",
    }
    return _special.get(k, pygame.key.name(k).upper())

def any_action_pressed(keys, action):
    for k in get_keybindings().get(action, []):
        try:
            if keys[k]:
                return True
        except (IndexError, TypeError):
            pass
    return False

def action_matches_key(event_key, action):
    return event_key in get_keybindings().get(action, [])

# =============================
# Menu Navigation (WASD/Pfeile, Space/Enter, Backspace/Esc/CapsLock)
# =============================
# Fixe UI-Konvention, bewusst NICHT in _DEFAULT_KB (draw_controls_menu() iteriert
# _DEFAULT_KB.keys() fürs Rebind-UI - alles dort würde umbindbar, auch die Fähigkeit
# selbst, Menüs überhaupt zu navigieren).

_NAV_UP, _NAV_DOWN   = (pygame.K_UP, pygame.K_w), (pygame.K_DOWN, pygame.K_s)
_NAV_LEFT, _NAV_RIGHT = (pygame.K_LEFT, pygame.K_a), (pygame.K_RIGHT, pygame.K_d)
_NAV_CONFIRM_KEYS = (pygame.K_SPACE, pygame.K_RETURN)
_NAV_BACK_KEYS    = (pygame.K_ESCAPE, pygame.K_CAPSLOCK)   # + Backspace, bedingt (siehe is_back_key)

# Der "Menü"-Tab im Steuerung-Screen macht die Navigations-Tasten oben ANZEIGBAR und
# ERGÄNZBAR, aber nicht ENTFERNBAR: _NAV_UP usw. bleiben immer zusätzlich aktiv, egal was
# hier gespeichert ist. Das verhindert, dass man sich durch Entfernen aller Tasten von der
# Menü-Navigation aussperrt (bis hin zu diesem Screen selbst) - _DEFAULT_KB/get_keybindings()
# wird hierfür bewusst NICHT genutzt, weil dort Tasten komplett entfernbar sind.
_MENU_NAV_DEFAULT_TUPLES = {
    "nav_up": _NAV_UP, "nav_down": _NAV_DOWN, "nav_left": _NAV_LEFT, "nav_right": _NAV_RIGHT,
    "nav_confirm": _NAV_CONFIRM_KEYS, "nav_back": _NAV_BACK_KEYS,
}

def get_menu_nav_extra_keys():
    extras = settings.setdefault("menu_nav_extra_keys", {})
    for _a in _MENU_NAV_ACTIONS:
        extras.setdefault(_a, [])
    return extras

def save_menu_nav_extra_keys(extras):
    settings["menu_nav_extra_keys"] = extras
    save_section("settings", settings)

_nav_focus = {}   # screen_id (str) -> int Fokusindex; lazy befüllt, kein expliziter Reset

# Fokus-Ring nur sichtbar, nachdem tatsächlich eine Pfeil-/WASD-Taste gedrückt wurde ("man ist
# in der Tastatur"); jede echte Mausbewegung (siehe get_mouse_pos_logical) schaltet ihn wieder
# aus, sodass Maus und Tastatur sich nicht gegenseitig ins Gehege kommen.
_kb_nav_active = False
_last_mouse_phys_pos = None

def nav_direction(key):
    d = None
    _extra = get_menu_nav_extra_keys()
    if key in _NAV_UP or key in _extra["nav_up"]: d = "up"
    elif key in _NAV_DOWN or key in _extra["nav_down"]: d = "down"
    elif key in _NAV_LEFT or key in _extra["nav_left"]: d = "left"
    elif key in _NAV_RIGHT or key in _extra["nav_right"]: d = "right"
    if d is not None:
        global _kb_nav_active
        _kb_nav_active = True
    return d

def is_nav_confirm(key):
    return key in _NAV_CONFIRM_KEYS or key in get_menu_nav_extra_keys()["nav_confirm"]

def is_back_key(key, allow_backspace=True):
    """allow_backspace=False an Stellen, wo Backspace bereits ein Zeichen in
    einem aktiven Textfeld löscht (Profilname, Mods-Menü-Textedit)."""
    if key in _NAV_BACK_KEYS or key in get_menu_nav_extra_keys()["nav_back"]:
        return True
    return allow_backspace and key == pygame.K_BACKSPACE

def nav_get_focus(screen_id, count, default_idx=0):
    if count <= 0:
        return None
    idx = _nav_focus.get(screen_id, default_idx)
    return idx if 0 <= idx < count else default_idx

def nav_set_focus(screen_id, idx):
    _nav_focus[screen_id] = idx

def nav_is_focused(screen_id, idx, count, default_idx=0):
    if not _kb_nav_active:
        return False
    return nav_get_focus(screen_id, count, default_idx) == idx

def nav_move(rects, current_idx, direction, lateral_weight=2.5):
    """Geometrische Nearest-Neighbour-Fokusbewegung. Ein Codepfad für Listen
    UND Grids: eine einspaltige Liste hat bei 'down' keine lateralen Kandidaten
    und reduziert sich automatisch auf next-in-list; ein mehrspaltiges Grid nach
    rechts findet die nächste Karte rechts. Kein Wrap-Around: am Ende einer
    Liste ist z.B. 'down' ein No-op statt Sprung nach oben (Layouts hier mischen
    Spalten/Zeilen/freistehende Buttons auf demselben Screen, "wrap" hätte keine
    einheitliche Bedeutung)."""
    if not rects:
        return current_idx
    if current_idx is None or not (0 <= current_idx < len(rects)):
        return 0
    cx, cy = rects[current_idx].center
    best_i, best_score = None, None
    for i, r in enumerate(rects):
        if i == current_idx:
            continue
        dx, dy = r.centerx - cx, r.centery - cy
        if   direction == "right" and dx > 0: primary, lateral = dx, dy
        elif direction == "left"  and dx < 0: primary, lateral = -dx, dy
        elif direction == "down"  and dy > 0: primary, lateral = dy, dx
        elif direction == "up"    and dy < 0: primary, lateral = -dy, dx
        else: continue
        score = primary + lateral_weight * abs(lateral)
        if best_score is None or score < best_score:
            best_score, best_i = score, i
    return best_i if best_i is not None else current_idx

def nav_reveal_delta(rect, fade_top, fade_bottom):
    """Für scrollbare Screens: wie viele Pixel ein Scroll-Offset sich ändern
    muss, damit `rect` (an der aktuellen Scroll-Position berechnet) vollständig
    sichtbar wird. Aufrufer addiert das auf die eigene Scroll-Variable und
    clampt genauso wie der bestehende Scrollbar-Code (0, wenn schon sichtbar)."""
    if rect.top < fade_top: return rect.top - fade_top
    if rect.bottom > fade_bottom: return rect.bottom - fade_bottom
    return 0

# Coins rules from your spec (mapping Hard=Schwer)
coin_rules = {
    "Schwer": 10,        # 10 Score = 1 Coin
    "Hardcore": 5,       # 5 Score = 1 Coin
    "Unmöglich": 3,      # 3 Score = 1 Coin
    "Albtraum": 2,       # 2 Score = 1 Coin
    "Hölle": 1.5         # ceil(Score / 1.5) = Coins
}
min_survive_seconds_for_coins = 15

# Load settings → indexes
language_index = languages.index(settings.get("language", "Deutsch")) if settings.get("language") in languages else 0
difficulty_index = difficulties.index(settings.get("difficulty", "Mittel")) if settings.get("difficulty") in difficulties else 1

# =============================
# Power System (Datenbank + Wahrscheinlichkeiten)
# =============================

# Farben für verschiedene Seltenheitsgrade der Powers
rarity_colors = {
    "Common": (0, 200, 0),      # Grün für Common
    "Rare": (80, 150, 255),     # Blau für Rare
    "Epic": (180, 80, 255),     # Lila für Epic
    "Legendary": (255, 200, 50), # Gold für Legendary
    "Mythic": (255, 140, 0)     # Orange für Mythic
}

# Datenbank aller verfügbaren Powers/Fähigkeiten
# Jede Power hat: rarity (Seltenheit), name (mehrsprachig), desc (kurze Beschreibung), detailed_desc (detaillierte Beschreibung)
POWERS_DB = {
    # Common (häufig)
    "Recovery": {
        "rarity": "Common",  # Seltenheitsgrad
        "name": {  # Mehrsprachige Namen
            "Deutsch": "Recovery",
            "English": "Recovery",
            "Русский": "Восстановление",
            "Türkçe": "Kurtarma",
            "Schwiizerdütsch": "Erholig"
        },
        "desc": {  # Kurze Beschreibungen pro Sprache
            "Deutsch": "+3 Coins wenn 10s überlebt (15s auf Schwer+)",
            "English": "+3 Coins if survive 10s (15s on Hard+)",
            "Русский": "+3 монеты если выжить 10с (15с на Сложно+)",
            "Türkçe": "+3 para eğer 10s hayatta kalırsan (15s Zor+)",
            "Schwiizerdütsch": "+3 Münze wenn 10s überlebt (15s uf Schwer+)"
        },
        "detailed_desc": {  # Detaillierte Beschreibungen pro Sprache
            "Deutsch": "Gewährt +3 Coins am Ende der Runde, wenn mindestens 10 Sekunden überlebt wurden (15s auf Schwer und höher). Keine Coins auf niedrigeren Schwierigkeiten.",
            "English": "Grants +3 Coins at the end of the run if survived at least 10 seconds (15s on Hard and above). No coins on lower difficulties.",
            "Русский": "Даёт +3 монеты в конце раунда, если выжил минимум 10 секунд (15с на Сложно и выше). Нет монет на более низких сложностях.",
            "Türkçe": "Tur sonunda en az 10 saniye hayatta kalırsan +3 para verir (15s Zor ve üstünde). Daha düşük zorluklarda para yok.",
            "Schwiizerdütsch": "Gitt +3 Münze am End vo de Rundi, wenn mindestens 10 Sekunde überlebt worde sind (15s uf Schwer und höcher). Kei Münze uf niedrigere Schwierigkeite."
        }
    },
    "Auto Dodge": {
        "rarity": "Common",
        "name": {
            "Deutsch": "Auto Dodge",
            "English": "Auto Dodge",
            "Русский": "Авто-уклонение",
            "Türkçe": "Otomatik Kaçınma",
            "Schwiizerdütsch": "Auto Dodge"
        },
        "desc": {
            "Deutsch": "5% Chance, Hindernisse zu umgehen",
            "English": "5% chance to dodge obstacles",
            "Русский": "5% шанс уклониться от препятствий",
            "Türkçe": "Engelleri otomatik kaçınma şansı %5",
            "Schwiizerdütsch": "5% Chance, Hindernisse z'umge"
        },
        "detailed_desc": {
            "Deutsch": "Gewährt eine 5% Chance, automatisch einem Hindernis auszuweichen, ohne Leben zu verlieren. Hilft bei unerwarteten Kollisionen.",
            "English": "Provides a 5% chance to automatically dodge an obstacle on collision without losing a life. Helps with unexpected hits.",
            "Русский": "Даёт 5% шанс автоматически уклониться от препятствия при столкновении без потери жизни. Помогает при неожиданных ударах.",
            "Türkçe": "Çarpışmada hayat kaybetmeden engelden otomatik olarak kaçınma şansı verir. Beklenmedik vuruşlarda yardımcı olur.",
            "Schwiizerdütsch": "Gitt e 5% Chance, automatisch em Hindernis usz'weiche, ohni Läbe z'verluure. Hilft bi unerwartete Kollisione."
        }
    },
    # Rare
    "Double Jump": {
        "rarity": "Rare",
        "name": {
            "Deutsch": "Double Jump",
            "English": "Double Jump",
            "Русский": "Двойной прыжок",
            "Türkçe": "Çift Zıplama",
            "Schwiizerdütsch": "Dopplet Springe"
        },
        "desc": {
            "Deutsch": "2x springen",
            "English": "Jump twice",
            "Русский": "Двойной прыжок",
            "Türkçe": "İki kez zıpla",
            "Schwiizerdütsch": "2x springe"
        },
        "detailed_desc": {
            "Deutsch": "Ermöglicht es dir, zweimal zu springen, bevor du den Boden berühren musst. Perfekt für das Überwinden großer Lücken und das Vermeiden von Hindernissen.",
            "English": "Allows you to jump twice before needing to touch the ground. Perfect for crossing large gaps and avoiding obstacles.",
            "Русский": "Позволяет прыгать дважды, прежде чем коснуться земли. Идеально для преодоления больших промежутков и избежания препятствий.",
            "Türkçe": "Yere değmeden önce iki kez zıplamanızı sağlar. Büyük boşlukları aşmak ve engelleri önlemek için mükemmel.",
            "Schwiizerdütsch": "Erlaubt dir, zweimal z'springe, bevor du de Bode berüere muest. Perfekt zum Überwinde vo grosse Lücke und zum Vermeide vo Hindernisse."
        }
    },
    "Slow Motion": {
        "rarity": "Rare",
        "name": {
            "Deutsch": "Slow Motion",
            "English": "Slow Motion",
            "Русский": "Замедление",
            "Türkçe": "Yavaş Hareket",
            "Schwiizerdütsch": "Langsami Bewegig"
        },
        "desc": {
            "Deutsch": "2s Gegner langsamer, CD 40s",
            "English": "2s enemies slower, CD 40s",
            "Русский": "2с враги медленнее, КД 40с",
            "Türkçe": "2s düşmanlar yavaş, CD 40s",
            "Schwiizerdütsch": "2s Gegner langsamer, CD 40s"
        },
        "detailed_desc": {
            "Deutsch": "Verlangsamt alle Gegner für 2 Sekunden auf die Hälfte ihrer Geschwindigkeit. Cooldown: 40 Sekunden. Gibt dir Zeit, durch enge Passagen zu navigieren.",
            "English": "Slows down all enemies to half speed for 2 seconds. Cooldown: 40 seconds. Gives you time to navigate through tight passages.",
            "Русский": "Замедляет всех врагов до половины скорости на 2 секунды. Перезарядка: 40 секунд. Даёт время для навигации через узкие проходы.",
            "Türkçe": "Tüm düşmanları 2 saniye boyunca yarı hızına yavaşlatır. Bekleme süresi: 40 saniye. Dar geçitlerden geçmek için size zaman verir.",
            "Schwiizerdütsch": "Verlangsamt alli Gegner für 2 Sekunde uf di Hälfti vo ihrer Gschwindigkeit. Cooldown: 40 Sekunde. Gitt dir Ziit, durch engi Passagen z'navigiere."
        },
        "cd": 40,
        "duration": 2
    },
    "Medic": {
        "rarity": "Rare",
        "name": {
            "Deutsch": "Medic",
            "English": "Medic",
            "Русский": "Медик",
            "Türkçe": "Doktor",
            "Schwiizerdütsch": "Sanitäter"
        },
        "desc": {
            "Deutsch": "+15% 1-UPs",
            "English": "+15% 1-UPs",
            "Русский": "+15% 1-ап",
            "Türkçe": "+15% 1-UP",
            "Schwiizerdütsch": "+15% 1-UP"
        },
        "detailed_desc": {
            "Deutsch": "Erhöht die Chance, 1-UP-Powerups zu finden, um 15%. Hilft dir, mehr Leben zu sammeln und länger zu überleben.",
            "English": "Increases the chance of finding 1-UP powerups by 15%. Helps you collect more lives and survive longer.",
            "Русский": "Увеличивает шанс найти 1-UP бонусы на 15%. Помогает собирать больше жизней и выживать дольше.",
            "Türkçe": "1-UP güçlendirmelerini bulma şansını %15 artırır. Daha fazla can toplamaya ve daha uzun süre hayatta kalmaya yardımcı olur.",
            "Schwiizerdütsch": "Erhöht di Chance, 1-UP-Powerups z'finde, um 15%. Hilft dir, meh Läbe z'sammle und länger z'überlebe."
        }
    },
    # Epic
    "Shield": {
        "rarity": "Epic",
        "name": {
            "Deutsch": "Shield",
            "English": "Shield",
            "Русский": "Щит",
            "Türkçe": "Kalkan",
            "Schwiizerdütsch": "Schild"
        },
        "desc": {
            "Deutsch": "Blockt 1 Hit, CD 40s",
            "English": "Blocks 1 hit every 40s",
            "Русский": "Блокирует 1 удар каждые 40с",
            "Türkçe": "40s'de 1 vuruş bloklar",
            "Schwiizerdütsch": "Blockt 1 Hit, CD 40s"
        },
        "detailed_desc": {
            "Deutsch": "Blockiert automatisch einen Treffer alle 40 Sekunden. Gibt dir einen kostenlosen Fehlschlag, ohne Leben zu verlieren.",
            "English": "Automatically blocks one hit every 40 seconds. Gives you a free miss without losing a life.",
            "Русский": "Автоматически блокирует одно попадание каждые 40 секунд. Даёт бесплатный промах без потери жизни.",
            "Türkçe": "Her 40 saniyede bir vuruşu otomatik olarak bloklar. Hayat kaybetmeden ücretsiz bir ıska verir.",
            "Schwiizerdütsch": "Blockiert automatisch ein Treffer alli 40 Sekunde. Gitt dir ein kostenlose Fehlschlag, ohni Läbe z'verluure."
        },
        "cd": 40
    },
    "Ghost Mode": {
        "rarity": "Epic",
        "name": {
            "Deutsch": "Ghost Mode",
            "English": "Ghost Mode",
            "Русский": "Режим призрака",
            "Türkçe": "Hayalet Modu",
            "Schwiizerdütsch": "Gäist-Modus"
        },
        "desc": {
            "Deutsch": "2s unverwundbar, CD 40s",
            "English": "2s invulnerable, CD 40s",
            "Русский": "2с неуязвим, КД 40с",
            "Türkçe": "2s hasarsız, CD 40s",
            "Schwiizerdütsch": "2s unverwundbar, CD 40s"
        },
        "detailed_desc": {
            "Deutsch": "Macht dich für 2 Sekunden unverwundbar gegen alle Hindernisse. Cooldown: 40 Sekunden. Perfekt für kritische Momente.",
            "English": "Makes you invulnerable to all obstacles for 2 seconds. Cooldown: 40 seconds. Perfect for critical moments.",
            "Русский": "Делает вас неуязвимым ко всем препятствиям на 2 секунды. Перезарядка: 40 секунд. Идеально для критических моментов.",
            "Türkçe": "Sizi tüm engellere karşı 2 saniye boyunca hasarsız kılar. Bekleme süresi: 40 saniye. Kritik anlar için mükemmel.",
            "Schwiizerdütsch": "Macht dich für 2 Sekunde unverwundbar gege alli Hindernisse. Cooldown: 40 Sekunde. Perfekt für kritische Momänt."
        },
        "cd": 40,
        "duration": (2, 2)
    },
    # Legendary
    "Nuke": {
        "rarity": "Legendary",
        "name": {
            "Deutsch": "Nuke",
            "English": "Nuke",
            "Русский": "Ядерка",
            "Türkçe": "Nükleer",
            "Schwiizerdütsch": "Atombombe"
        },
        "desc": {
            "Deutsch": "Zerstört Hindernisse um Spieler für 4s, CD 20s",
            "English": "Destroys obstacles around player for 4s, CD 20s",
            "Русский": "Уничтожает препятствия вокруг игрока на 4с, КД 20с",
            "Türkçe": "Oyuncu etrafındaki engelleri 4s yok eder, CD 20s",
            "Schwiizerdütsch": "Zerstört Hindernisse um Spieler für 4s, CD 20s"
        },
        "detailed_desc": {
            "Deutsch": "Zerstört für 4 Sekunden alle Hindernisse in der Nähe des Spielers. Cooldown: 20 Sekunden. Rettet dich aus verzweifelten Situationen.",
            "English": "Destroys all obstacles near the player for 4 seconds. Cooldown: 20 seconds. Saves you from desperate situations.",
            "Русский": "Уничтожает все препятствия рядом с игроком на 4 секунды. Перезарядка: 20 секунд. Спасает от отчаянных ситуаций.",
            "Türkçe": "Oyuncunun yakınındaki tüm engelleri 4 saniye boyunca yok eder. Bekleme süresi: 20 saniye. Umutsuz durumları kurtarır.",
            "Schwiizerdütsch": "Zerstört für 4 Sekunde alli Hindernisse in der Nähe vom Spieler. Cooldown: 20 Sekunde. Rettet dich us verzweifelte Situatione."
        },
        "cd": 20
    },
    "Extra Slot": {
        "rarity": "Legendary",
        "name": {
            "Deutsch": "Extra Slot",
            "English": "Extra Slot",
            "Русский": "Дополнительный слот",
            "Türkçe": "Ek Yuva",
            "Schwiizerdütsch": "Extra Slot"
        },
        "desc": {
            "Deutsch": "+1 Power-Slot",
            "English": "+1 Power slot",
            "Русский": "+1 слот силы",
            "Türkçe": "+1 güç yuvası",
            "Schwiizerdütsch": "+1 Power-Slot"
        },
        "detailed_desc": {
            "Deutsch": "Fügt einen zusätzlichen Power-Slot hinzu. Erlaubt dir, mehr Powers gleichzeitig zu nutzen.",
            "English": "Adds an extra power slot. Allows you to use more powers simultaneously.",
            "Русский": "Добавляет дополнительный слот силы. Позволяет использовать больше сил одновременно.",
            "Türkçe": "Ekstra bir güç yuvası ekler. Aynı anda daha fazla güç kullanmanıza izin verir.",
            "Schwiizerdütsch": "Füegt ein zusätzliche Power-Slot hinzu. Erlaubt dir, meh Powers gleichzeitig z'nutze."
        }
    },
    "Time Freeze": {
        "rarity": "Legendary",
        "name": {
            "Deutsch": "Time Freeze",
            "English": "Time Freeze",
            "Русский": "Заморозка времени",
            "Türkçe": "Zaman Dondurma",
            "Schwiizerdütsch": "Ziit Friere"
        },
        "desc": {
            "Deutsch": "2s Freeze + 5s Slow, CD 30s",
            "English": "2s Freeze + 5s Slow, CD 30s",
            "Русский": "2с заморозка + 5с замедление, КД 30с",
            "Türkçe": "2s dondur + 5s yavaş, CD 30s",
            "Schwiizerdütsch": "2s Freeze + 5s Slow, CD 30s"
        },
        "detailed_desc": {
            "Deutsch": "Friert die Zeit für 2 Sekunden ein und verlangsamt sie danach für 5 Sekunden. Cooldown: 30 Sekunden. Gibt dir maximale Kontrolle.",
            "English": "Freezes time for 2 seconds and slows it down for 5 seconds after. Cooldown: 30 seconds. Gives you maximum control.",
            "Русский": "Замораживает время на 2 секунды и замедляет его на 5 секунд после. Перезарядка: 30 секунд. Даёт максимальный контроль.",
            "Türkçe": "Zamanı 2 saniye dondurur ve sonrasında 5 saniye yavaşlatır. Bekleme süresi: 30 saniye. Size maksimum kontrol verir.",
            "Schwiizerdütsch": "Friert di Ziit für 2 Sekunde ei und verlangsamt si danach für 5 Sekunde. Cooldown: 30 Sekunde. Gitt dir maximale Kontrolle."
        },
        "cd": 30
    },
    "Phoenix": {
        "rarity": "Mythic",
        "name": {
            "Deutsch": "Phoenix",
            "English": "Phoenix",
            "Русский": "Феникс",
            "Türkçe": "Anka",
            "Schwiizerdütsch": "Phoenix"
        },
        "desc": {
            "Deutsch": "Passiv: Wiederbelebung bei Tod (2min CD)",
            "English": "Passive: Revival on death (2min CD)",
            "Русский": "Пассивно: воскрешение при смерти (КД 2мин)",
            "Türkçe": "Pasif: Ölümde dirilme (2dk CD)",
            "Schwiizerdütsch": "Passiv: Uferstehung bi Tod (2min CD)"
        },
        "detailed_desc": {
            "Deutsch": "Passives Wiederbelebungssystem. Bei lethalem Schaden wird der Spieler mit 2 HP wiederbelebt, erhält 2 Sekunden Unverwundbarkeit und das Spiel wird 6 Sekunden verlangsamt. Cooldown: 2 Minuten.",
            "English": "Passive revival system. On lethal damage, player is revived with 2 HP, gains 2 seconds of invulnerability, and the game slows down for 6 seconds. Cooldown: 2 minutes.",
            "Русский": "Пассивная система воскрешения. При летальном уроне игрок воскрешается с 2 HP, получает 2 секунды неуязвимости, и игра замедляется на 6 секунд. Перезарядка: 2 минуты.",
            "Türkçe": "Pasif dirilme sistemi. Ölümcül hasarda oyuncu 2 HP ile dirilir, 2 saniye hasarsızlık kazanır ve oyun 6 saniye yavaşlar. Bekleme süresi: 2 dakika.",
            "Schwiizerdütsch": "Passivi Uferstehungssystem. Bi letalem Schade wird de Spieler mit 2 HP ufersteut, erhalt 2 Sekunde Unverwundbarkeit und s Spiel wird 6 Sekunde verlangsamt. Cooldown: 2 Minute."
        }
    },
    "Magnet": {
        "rarity": "Rare",
        "name": {
            "Deutsch": "Magnet",
            "English": "Magnet",
            "Русский": "Магнит",
            "Türkçe": "Mıknatıs",
            "Schwiizerdütsch": "Magnet"
        },
        "desc": {
            "Deutsch": "Zieht 1-UPs aus der Nähe an",
            "English": "Pulls nearby 1-UPs toward you",
            "Русский": "Притягивает ближние 1-UP",
            "Türkçe": "Yakındaki 1-UP'ları çeker",
            "Schwiizerdütsch": "Zieht 1-UPs i de Nöchi a"
        },
        "detailed_desc": {
            "Deutsch": "Passiv: Zieht 1-UP-Powerups automatisch in einem Radius zum Spieler, sodass sie leichter eingesammelt werden können.",
            "English": "Passive: Automatically pulls 1-UP powerups within a radius toward the player, making them easier to collect.",
            "Русский": "Пассивно: автоматически притягивает 1-UP усиления в радиусе к игроку, облегчая их сбор.",
            "Türkçe": "Pasif: Belirli bir yarıçaptaki 1-UP güçlendirmelerini otomatik olarak oyuncuya çeker, toplamayı kolaylaştırır.",
            "Schwiizerdütsch": "Passiv: Zieht 1-UP-Powerups automatisch i eme Radius zum Spieler, so dass sie liechter iigsammlet werde chönd."
        }
    }
}

# Wahrscheinlichkeiten für das Rollen von Powers verschiedener Seltenheit (gewichtete Werte)
# Summe = 1000, also Wahrscheinlichkeiten: Common 79.5%, Rare 10%, Epic 8%, Legendary 2%, Mythic 0.5%
RARITY_PROBS = {"Common": 795, "Rare": 100, "Epic": 80, "Legendary": 20, "Mythic": 5}


def _strip_unknown_powers(state):
    """Entfernt Power-Namen aus owned/equipped, die es in POWERS_DB nicht (mehr) gibt -
    z.B. Geister-Einträge nach dem Entfernen einer Power (Coin Booster) aus alten Spielständen."""
    changed = False
    for key in ("owned", "equipped"):
        lst = state.get(key)
        if isinstance(lst, list):
            cleaned = [name for name in lst if name in POWERS_DB]
            if cleaned != lst:
                state[key] = cleaned
                changed = True
    return state, changed


# Bestehenden, schon geladenen powers_state direkt beim Start bereinigen und - falls sich
# was geändert hat - einmalig zurückspeichern (POWERS_DB steht hier oben erst fest, daher
# nicht schon bei load_section() weiter oben möglich).
powers_state, _powers_changed = _strip_unknown_powers(powers_state)
if _powers_changed:
    save_section("powers", powers_state)



# =============================
# COSMETICS DB (Kosmetik-Items wie Trails)
# Jedes Item hat: rarity (Seltenheit), price_range (Preisbereich min-max),
# type (trail/player), name (mehrsprachig)
# Preise werden beim ersten Laden zufällig festgelegt und in cosmetics.json gespeichert
# =============================
COSMETICS_DB = {
    "fire_trail": {"rarity": "Common", "price_range": [150, 250], "type": "trail", "category": "trail", "name": {"Deutsch": "Feuerspur", "English": "Fire Trail", "Русский": "огненный след", "Türkçe": "ateşli iz", "Schwiizerdütsch": "Feierig Spur"}},
    "ice_trail": {"rarity": "Rare", "price_range": [200, 300], "type": "trail", "category": "trail", "name": {"Deutsch": "Eisspur", "English": "Ice Trail", "Русский": "ледяной след", "Türkçe": "Buz İz", "Schwiizerdütsch": "Iis Spur"}},
    "shadow_trail": {"rarity": "Epic", "price_range": [250, 400], "type": "trail", "category": "trail", "name": {"Deutsch": "Schattenspur", "English": "Shadow Trail", "Русский": "тень след", "Türkçe": "Gölge İz", "Schwiizerdütsch": "Schatte Spur"}},
    "sparkle_trail": {"rarity": "Epic", "price_range": [350, 500], "type": "trail", "category": "trail", "name": {"Deutsch": "Funkelspur", "English": "Sparkle Trail", "Русский": "искрящийся след", "Türkçe": "Parıltı İz", "Schwiizerdütsch": "Funkle Spur"}},
    "toxic_trail": {"rarity": "Epic", "price_range": [400, 600], "type": "trail", "category": "trail", "name": {"Deutsch": "Giftspur", "English": "Toxic Trail", "Русский": "токсичный след", "Türkçe": "Toksik İz", "Schwiizerdütsch": "Gift Spur"}},
    "electric_trail": {"rarity": "Epic", "price_range": [450, 650], "type": "trail", "category": "trail", "name": {"Deutsch": "Blitzspur", "English": "Electric Trail", "Русский": "электрический след", "Türkçe": "Elektrik İzi", "Schwiizerdütsch": "Blitz Spur"}},
    "yellow_crown": {"rarity": "Legendary", "price_range": [2500, 2500], "type": "player", "category": "headwear", "name": {"Deutsch": "Gelbe Krone", "English": "Yellow Crown", "Русский": "желтая корона", "Türkçe": "Sarı Taç", "Schwiizerdütsch": "Gäl Krone"}},
    "pixel_eyes": {"rarity": "Common", "price_range": [120, 200], "type": "player", "category": "eyes", "name": {"Deutsch": "Pixel-Augen", "English": "Pixel Eyes", "Русский": "Пиксельные глаза", "Türkçe": "Piksel Gözler", "Schwiizerdütsch": "Pixel-Ouge"}},
    "top_hat": {"rarity": "Rare", "price_range": [280, 380], "type": "player", "category": "headwear", "name": {"Deutsch": "Zylinder", "English": "Top Hat", "Русский": "Цилиндр", "Türkçe": "Silindir Şapka", "Schwiizerdütsch": "Zylinder"}},
    "devil_horns": {"rarity": "Epic", "price_range": [420, 580], "type": "player", "category": "headwear", "name": {"Deutsch": "Teufelshörner", "English": "Devil Horns", "Русский": "Дьявольские рога", "Türkçe": "Şeytan Boynuzları", "Schwiizerdütsch": "Tüfels-Hörner"}},
    "clown_nose": {"rarity": "Common", "price_range": [100, 150], "type": "player", "category": "nose", "name": {"Deutsch": "Clown-Nase", "English": "Clown Nose", "Русский": "Клоунский нос", "Türkçe": "Palyaço Burnu", "Schwiizerdütsch": "Clown-Nase"}},
    "big_grin": {"rarity": "Rare", "price_range": [220, 320], "type": "player", "category": "mouth", "name": {"Deutsch": "Breites Grinsen", "English": "Big Grin", "Русский": "Широкая ухмылка", "Türkçe": "Geniş Sırıtış", "Schwiizerdütsch": "Breits Grinse"}},
    "rainbow_trail": {"rarity": "Legendary", "price_range": [800, 1200], "type": "trail", "category": "trail", "name": {"Deutsch": "Regenbogenspur", "English": "Rainbow Trail", "Русский": "радужный след", "Türkçe": "Gökkuşağı İzi", "Schwiizerdütsch": "Regeboge Spur"}},
    "star_trail": {"rarity": "Rare", "price_range": [220, 320], "type": "trail", "category": "trail", "name": {"Deutsch": "Sternenspur", "English": "Star Trail", "Русский": "звёздный след", "Türkçe": "Yıldız İzi", "Schwiizerdütsch": "Sternespur"}},
    "smoke_trail": {"rarity": "Common", "price_range": [100, 180], "type": "trail", "category": "trail", "name": {"Deutsch": "Rauchspur", "English": "Smoke Trail", "Русский": "дымный след", "Türkçe": "Duman İzi", "Schwiizerdütsch": "Rauch Spur"}},
    "blood_trail": {"rarity": "Epic", "price_range": [350, 500], "type": "trail", "category": "trail", "name": {"Deutsch": "Blutspur", "English": "Blood Trail", "Русский": "кровавый след", "Türkçe": "Kan İzi", "Schwiizerdütsch": "Bluetspur"}},
    "heart_trail": {"rarity": "Rare", "price_range": [200, 300], "type": "trail", "category": "trail", "name": {"Deutsch": "Herzspur", "English": "Heart Trail", "Русский": "след из сердец", "Türkçe": "Kalp İzi", "Schwiizerdütsch": "Härzspur"}},

    # --- Kopfbedeckungen-Erweiterung ---
    "halo": {"rarity": "Legendary", "price_range": [2000, 2500], "type": "player", "category": "headwear", "name": {"Deutsch": "Heiligenschein", "English": "Halo", "Русский": "Нимб", "Türkçe": "Hâle", "Schwiizerdütsch": "Heiligeschii"}},
    "propeller_hat": {"rarity": "Rare", "price_range": [250, 350], "type": "player", "category": "headwear", "name": {"Deutsch": "Propellermütze", "English": "Propeller Hat", "Русский": "Шапка с пропеллером", "Türkçe": "Pervaneli Şapka", "Schwiizerdütsch": "Propeller-Mütz"}},
    "ninja_headband": {"rarity": "Common", "price_range": [120, 200], "type": "player", "category": "headwear", "name": {"Deutsch": "Ninja-Stirnband", "English": "Ninja Headband", "Русский": "Ниндзя-повязка", "Türkçe": "Ninja Bandanası", "Schwiizerdütsch": "Ninja-Stirnband"}},

    # --- Augen-Erweiterung ---
    "sunglasses": {"rarity": "Common", "price_range": [130, 200], "type": "player", "category": "eyes", "name": {"Deutsch": "Sonnenbrille", "English": "Sunglasses", "Русский": "Солнцезащитные очки", "Türkçe": "Güneş Gözlüğü", "Schwiizerdütsch": "Sunnebrülle"}},
    "3d_glasses": {"rarity": "Rare", "price_range": [200, 300], "type": "player", "category": "eyes", "name": {"Deutsch": "3D-Brille", "English": "3D Glasses", "Русский": "3D-очки", "Türkçe": "3D Gözlük", "Schwiizerdütsch": "3D-Brülle"}},
    "robot_visor": {"rarity": "Epic", "price_range": [420, 580], "type": "player", "category": "eyes", "name": {"Deutsch": "Roboter-Visier", "English": "Robot Visor", "Русский": "Роботизированный визор", "Türkçe": "Robot Vizörü", "Schwiizerdütsch": "Roboter-Visier"}},

    # --- Nasen- & Mund-Erweiterung ---
    "pig_nose": {"rarity": "Rare", "price_range": [180, 260], "type": "player", "category": "nose", "name": {"Deutsch": "Schweinenase", "English": "Pig Nose", "Русский": "Свиной нос", "Türkçe": "Domuz Burnu", "Schwiizerdütsch": "Söili-Nase"}},
    "bird_beak": {"rarity": "Rare", "price_range": [190, 270], "type": "player", "category": "mouth", "name": {"Deutsch": "Vogelschnabel", "English": "Bird Beak", "Русский": "Птичий клюв", "Türkçe": "Kuş Gagası", "Schwiizerdütsch": "Vogel-Schnabel"}},
    "vampire_fangs": {"rarity": "Epic", "price_range": [380, 520], "type": "player", "category": "mouth", "name": {"Deutsch": "Vampirzähne", "English": "Vampire Fangs", "Русский": "Вампирские клыки", "Türkçe": "Vampir Dişleri", "Schwiizerdütsch": "Vampir-Zähn"}},
    "nerd_teeth": {"rarity": "Common", "price_range": [110, 170], "type": "player", "category": "mouth", "name": {"Deutsch": "Nerd-Hasenzähne", "English": "Nerd Buck Teeth", "Русский": "Кроличьи зубы ботана", "Türkçe": "İnek Tavşan Dişleri", "Schwiizerdütsch": "Nerd-Häsezähn"}}
}

# Zentrale Liste aller Trail-Item-Keys, damit Partikel-Logik & Ausrüstungs-Checks
# nicht bei jedem neuen Trail an mehreren Stellen manuell erweitert werden müssen.
ALL_TRAIL_KEYS = [key for key, meta in COSMETICS_DB.items() if meta.get("category") == "trail"]

# =============================
# Gemeinsame Kosmetik-Optik (Rarität-Farben & Icon-Symbole)
# Zentral definiert, damit Shop UND Inventar exakt dieselbe, hübsche
# Kartenoptik (Farben je Seltenheit + Vektor-Icons) verwenden.
# =============================
RARITY_COLORS = {
    "Common":    {"bg": (60, 130, 60),   "border": (100, 220, 100),  "badge": (80, 200, 80),   "label": "Common"},
    "Rare":      {"bg": (40, 80, 180),   "border": (80, 140, 255),   "badge": (70, 130, 240),  "label": "Rare"},
    "Epic":      {"bg": (100, 40, 160),  "border": (180, 80, 255),   "badge": (160, 70, 240),  "label": "Epic"},
    "Legendary": {"bg": (160, 110, 10),  "border": (255, 200, 0),    "badge": (220, 170, 20),  "label": "Legendary"},
}

def get_rarity_colors(rarity):
    """Gibt die Farbdefinition für eine Seltenheitsstufe zurück (Fallback: Common)."""
    return RARITY_COLORS.get(rarity, RARITY_COLORS["Common"])

# Icon draw functions per item (shape, color) — keine Emojis, reine Vektorformen
ITEM_ICONS = {
    "fire_trail":       ("triangle",  (255, 120, 30)),
    "ice_trail":        ("diamond",   (140, 210, 255)),
    "shadow_trail":     ("circle",    (60,  40,  80)),
    "sparkle_trail":    ("star4",     (255, 240, 80)),
    "toxic_trail":      ("hexagon",   (80,  210, 60)),
    "electric_trail":   ("bolt",      (80,  220, 255)),
    "yellow_crown":     ("crown",     (255, 215, 0)),
    "pixel_eyes":       ("eyes",      (60,  60,  70)),
    "top_hat":          ("tophat",    (25,  25,  30)),
    "devil_horns":      ("horns",     (210, 30,  30)),
    "clown_nose":       ("nose",      (230, 30,  30)),
    "big_grin":         ("grin",      (255, 90,  120)),
    "rainbow_trail":    ("rainbow",   (255, 255, 255)),
    "star_trail":       ("star4",     (210, 230, 255)),
    "smoke_trail":      ("cloud",     (170, 170, 175)),
    "blood_trail":      ("drop",      (150, 10,  20)),
    "heart_trail":      ("heart",     (230, 40,  90)),
    "halo":             ("halo",      (255, 200, 20)),
    "propeller_hat":    ("propeller", (230, 70,  60)),
    "ninja_headband":   ("headband",  (25,  25,  30)),
    "sunglasses":       ("sunglasses",(20,  20,  25)),
    "3d_glasses":       ("3d_glasses",(230, 30,  30)),
    "robot_visor":      ("robot_visor",(80, 220, 255)),
    "pig_nose":         ("pig_nose",  (240, 150, 170)),
    "bird_beak":        ("bird_beak", (240, 170, 30)),
    "vampire_fangs":    ("vampire_fangs",(255, 255, 250)),
    "nerd_teeth":       ("nerd_teeth",(255, 255, 250)),
}

_COSMETIC_CATEGORY_TO_SLOT = {
    "headwear": "equipped_headwear", "eyes": "equipped_eyes",
    "nose": "equipped_nose", "mouth": "equipped_mouth",
}

def draw_cosmetic_icon(surface, key, icx, icy, big=False):
    """Zeichnet das Vektor-Icon eines Kosmetik-Items zentriert bei (icx, icy).
    'big' skaliert die Formen für die große Vorschau (Shop/Inventar-Detailkarten)."""
    shape, ic = ITEM_ICONS.get(key, ("circle", (200, 200, 200)))
    if shape == "circle":
        pygame.draw.circle(surface, ic, (icx, icy), 16 if big else 9)
    elif shape == "triangle":
        pts = [(icx, icy - (18 if big else 10)), (icx - (16 if big else 9), icy + (13 if big else 7)), (icx + (16 if big else 9), icy + (13 if big else 7))]
        pygame.draw.polygon(surface, ic, pts)
    elif shape == "diamond":
        d = 18 if big else 10
        pts = [(icx, icy - d), (icx + d, icy), (icx, icy + d), (icx - d, icy)]
        pygame.draw.polygon(surface, ic, pts)
    elif shape == "star4":
        o, i2 = (20, 7) if big else (11, 4)
        pts = [(icx, icy - o), (icx + i2, icy - i2), (icx + o, icy),
               (icx + i2, icy + i2), (icx, icy + o), (icx - i2, icy + i2),
               (icx - o, icy), (icx - i2, icy - i2)]
        pygame.draw.polygon(surface, ic, pts)
    elif shape == "hexagon":
        r = 18 if big else 10
        pts = [(icx + int(r * math.cos(math.radians(60 * n - 30))),
                icy + int(r * math.sin(math.radians(60 * n - 30)))) for n in range(6)]
        pygame.draw.polygon(surface, ic, pts)
    elif shape == "crown":
        if big:
            pygame.draw.rect(surface, ic, (icx - 16, icy + 4, 32, 12))
            pygame.draw.polygon(surface, ic, [(icx - 16, icy + 4), (icx - 16, icy - 14), (icx - 6, icy - 4)])
            pygame.draw.polygon(surface, ic, [(icx, icy + 4), (icx, icy - 18), (icx + 8, icy - 4)])
            pygame.draw.polygon(surface, ic, [(icx + 16, icy + 4), (icx + 16, icy - 14), (icx + 6, icy - 4)])
        else:
            pygame.draw.rect(surface, ic, (icx - 9, icy + 2, 18, 7))
            pygame.draw.polygon(surface, ic, [(icx - 9, icy + 2), (icx - 9, icy - 8), (icx - 4, icy - 2)])
            pygame.draw.polygon(surface, ic, [(icx, icy + 2), (icx, icy - 10), (icx + 5, icy - 2)])
            pygame.draw.polygon(surface, ic, [(icx + 9, icy + 2), (icx + 9, icy - 8), (icx + 4, icy - 2)])
    elif shape == "bolt":
        if big:
            pts = [(icx + 5, icy - 19), (icx - 9, icy + 2), (icx - 2, icy + 2),
                   (icx - 7, icy + 19), (icx + 10, icy - 3), (icx + 2, icy - 3)]
        else:
            pts = [(icx + 3, icy - 11), (icx - 5, icy + 1), (icx - 1, icy + 1),
                   (icx - 4, icy + 11), (icx + 6, icy - 2), (icx + 1, icy - 2)]
        pygame.draw.polygon(surface, ic, pts)
    elif shape == "eyes":
        if big:
            pygame.draw.rect(surface, (255, 255, 255), (icx - 14, icy - 7, 10, 10))
            pygame.draw.rect(surface, (255, 255, 255), (icx + 4, icy - 7, 10, 10))
            pygame.draw.rect(surface, ic, (icx - 11, icy - 4, 4, 4))
            pygame.draw.rect(surface, ic, (icx + 7, icy - 4, 4, 4))
        else:
            pygame.draw.rect(surface, (255, 255, 255), (icx - 8, icy - 4, 6, 6))
            pygame.draw.rect(surface, (255, 255, 255), (icx + 2, icy - 4, 6, 6))
            pygame.draw.rect(surface, ic, (icx - 6, icy - 2, 2, 2))
            pygame.draw.rect(surface, ic, (icx + 4, icy - 2, 2, 2))
    elif shape == "tophat":
        if big:
            pygame.draw.rect(surface, ic, (icx - 16, icy + 7, 32, 7))
            pygame.draw.rect(surface, ic, (icx - 11, icy - 17, 22, 26))
            pygame.draw.rect(surface, (150, 20, 20), (icx - 11, icy + 2, 22, 5))
        else:
            pygame.draw.rect(surface, ic, (icx - 9, icy + 4, 18, 4))
            pygame.draw.rect(surface, ic, (icx - 6, icy - 10, 12, 15))
            pygame.draw.rect(surface, (150, 20, 20), (icx - 6, icy + 1, 12, 3))
    elif shape == "horns":
        if big:
            pygame.draw.polygon(surface, ic, [(icx - 4, icy + 14), (icx - 19, icy - 10), (icx - 7, icy + 5)])
            pygame.draw.polygon(surface, ic, [(icx + 4, icy + 14), (icx + 19, icy - 10), (icx + 7, icy + 5)])
        else:
            pygame.draw.polygon(surface, ic, [(icx - 2, icy + 8), (icx - 11, icy - 6), (icx - 4, icy + 3)])
            pygame.draw.polygon(surface, ic, [(icx + 2, icy + 8), (icx + 11, icy - 6), (icx + 4, icy + 3)])
    elif shape == "nose":
        r = 14 if big else 8
        pygame.draw.circle(surface, ic, (icx, icy), r)
        pygame.draw.circle(surface, tuple(max(0, c - 60) for c in ic), (icx, icy), r, width=2)
    elif shape == "grin":
        px = 3 if big else 1
        pat = ["1..........1", "1..........1", "1..........1", "1..........1", "1..........1", "111111111111"]
        for pr, prow in enumerate(pat):
            for pc, pch in enumerate(prow):
                if pch == "1":
                    pygame.draw.rect(surface, (10, 10, 15),
                                      (icx - 13 * px // 2 + pc * px, icy - 3 * px + pr * px, px, px))
    elif shape == "rainbow":
        cols = [(230, 60, 60), (240, 160, 40), (230, 220, 60), (90, 200, 90), (70, 140, 230)]
        if big:
            for ri, rc2 in enumerate(cols):
                pygame.draw.arc(surface, rc2, (icx - 20, icy - 10 + ri * 2, 40, 40 - ri * 4), 3.14159, 2 * 3.14159, 3)
        else:
            for ri, rc2 in enumerate(cols):
                pygame.draw.arc(surface, rc2, (icx - 12, icy - 6 + ri, 24, 24 - ri * 2), 3.14159, 2 * 3.14159, 2)
    elif shape == "cloud":
        r1, r2, dy = (10, 12, 4) if big else (6, 7, 2)
        pygame.draw.circle(surface, ic, (icx - (10 if big else 6), icy + dy), r1)
        pygame.draw.circle(surface, ic, (icx + (10 if big else 6), icy + dy), r1)
        pygame.draw.circle(surface, ic, (icx, icy - (5 if big else 3)), r2)
    elif shape == "drop":
        if big:
            pygame.draw.polygon(surface, ic, [(icx, icy - 18), (icx - 14, icy + 6), (icx + 14, icy + 6)])
            pygame.draw.circle(surface, ic, (icx, icy + 8), 12)
        else:
            pygame.draw.polygon(surface, ic, [(icx, icy - 10), (icx - 8, icy + 4), (icx + 8, icy + 4)])
            pygame.draw.circle(surface, ic, (icx, icy + 5), 7)
    elif shape == "heart":
        if big:
            pygame.draw.circle(surface, ic, (icx - 9, icy - 5), 11)
            pygame.draw.circle(surface, ic, (icx + 9, icy - 5), 11)
            pygame.draw.polygon(surface, ic, [(icx - 18, icy - 2), (icx + 18, icy - 2), (icx, icy + 18)])
        else:
            pygame.draw.circle(surface, ic, (icx - 5, icy - 3), 6)
            pygame.draw.circle(surface, ic, (icx + 5, icy - 3), 6)
            pygame.draw.polygon(surface, ic, [(icx - 10, icy - 1), (icx + 10, icy - 1), (icx, icy + 10)])
    elif shape == "halo":
        # Goldgelber, runder Ring über dem Kopf
        if big:
            pygame.draw.ellipse(surface, ic, (icx - 15, icy - 10, 30, 20), width=4)
        else:
            pygame.draw.ellipse(surface, ic, (icx - 9, icy - 6, 18, 12), width=3)
    elif shape == "propeller":
        # Mütze mit Propeller-Kreuz obendrauf
        if big:
            pygame.draw.polygon(surface, ic, [(icx - 16, icy + 14), (icx + 16, icy + 14), (icx + 10, icy - 2), (icx - 10, icy - 2)])
            pygame.draw.line(surface, (60, 60, 65), (icx - 18, icy - 12), (icx + 18, icy - 12), 3)
            pygame.draw.line(surface, (60, 60, 65), (icx, icy - 24), (icx, icy - 2), 3)
            pygame.draw.circle(surface, (255, 210, 40), (icx, icy - 12), 4)
        else:
            pygame.draw.polygon(surface, ic, [(icx - 9, icy + 8), (icx + 9, icy + 8), (icx + 6, icy - 2), (icx - 6, icy - 2)])
            pygame.draw.line(surface, (60, 60, 65), (icx - 10, icy - 7), (icx + 10, icy - 7), 2)
            pygame.draw.line(surface, (60, 60, 65), (icx, icy - 13), (icx, icy - 2), 2)
            pygame.draw.circle(surface, (255, 210, 40), (icx, icy - 7), 2)
    elif shape == "headband":
        # Stirnband mit fliegenden Enden (Ninja-Style)
        if big:
            pygame.draw.rect(surface, ic, (icx - 18, icy - 5, 36, 11))
            pygame.draw.polygon(surface, (190, 20, 20), [(icx + 14, icy + 6), (icx + 28, icy + 18), (icx + 16, icy + 18)])
            pygame.draw.circle(surface, (190, 20, 20), (icx, icy), 3)
        else:
            pygame.draw.rect(surface, ic, (icx - 11, icy - 3, 22, 7))
            pygame.draw.polygon(surface, (190, 20, 20), [(icx + 8, icy + 4), (icx + 16, icy + 11), (icx + 10, icy + 11)])
            pygame.draw.circle(surface, (190, 20, 20), (icx, icy), 2)
    elif shape == "helmet":
        # Runder Helm mit blau schimmerndem Visier
        if big:
            pygame.draw.circle(surface, ic, (icx, icy), 19)
            pygame.draw.ellipse(surface, (140, 210, 255), (icx - 11, icy - 10, 22, 18))
            pygame.draw.ellipse(surface, (255, 255, 255), (icx - 11, icy - 10, 22, 18), width=2)
        else:
            pygame.draw.circle(surface, ic, (icx, icy), 11)
            pygame.draw.ellipse(surface, (140, 210, 255), (icx - 6, icy - 6, 13, 11))
            pygame.draw.ellipse(surface, (255, 255, 255), (icx - 6, icy - 6, 13, 11), width=1)
    elif shape == "sunglasses":
        # Runde Gläser mit dickem Rahmen und diagonalem Glanzstreifen
        if big:
            pygame.draw.circle(surface, ic, (icx - 10, icy), 11)
            pygame.draw.circle(surface, ic, (icx + 10, icy), 11)
            pygame.draw.circle(surface, (30, 30, 35), (icx - 10, icy), 8)
            pygame.draw.circle(surface, (30, 30, 35), (icx + 10, icy), 8)
            pygame.draw.line(surface, (140, 170, 200), (icx - 14, icy - 4), (icx - 7, icy + 3), 2)
            pygame.draw.line(surface, (140, 170, 200), (icx + 6, icy - 4), (icx + 13, icy + 3), 2)
            pygame.draw.rect(surface, ic, (icx - 3, icy - 2, 6, 4))
            pygame.draw.line(surface, ic, (icx - 20, icy - 2), (icx - 21, icy - 6), 2)
            pygame.draw.line(surface, ic, (icx + 20, icy - 2), (icx + 21, icy - 6), 2)
        else:
            pygame.draw.circle(surface, ic, (icx - 6, icy), 7)
            pygame.draw.circle(surface, ic, (icx + 6, icy), 7)
            pygame.draw.circle(surface, (30, 30, 35), (icx - 6, icy), 5)
            pygame.draw.circle(surface, (30, 30, 35), (icx + 6, icy), 5)
            pygame.draw.line(surface, (140, 170, 200), (icx - 9, icy - 2), (icx - 5, icy + 2), 1)
            pygame.draw.line(surface, (140, 170, 200), (icx + 3, icy - 2), (icx + 7, icy + 2), 1)
            pygame.draw.rect(surface, ic, (icx - 2, icy - 1, 4, 2))
    elif shape == "3d_glasses":
        # Klassische 3D-Brille: ein rotes und ein blaues Glas
        if big:
            pygame.draw.rect(surface, (200, 30, 30), (icx - 17, icy - 8, 14, 12))
            pygame.draw.rect(surface, (40, 120, 220), (icx + 3, icy - 8, 14, 12))
            pygame.draw.rect(surface, (20, 20, 25), (icx - 4, icy - 3, 8, 3))
        else:
            pygame.draw.rect(surface, (200, 30, 30), (icx - 10, icy - 5, 8, 7))
            pygame.draw.rect(surface, (40, 120, 220), (icx + 2, icy - 5, 8, 7))
            pygame.draw.rect(surface, (20, 20, 25), (icx - 2, icy - 2, 4, 2))
    elif shape == "robot_visor":
        # Durchgehendes, leuchtendes Visier über beiden Augen
        if big:
            pygame.draw.rect(surface, (30, 30, 35), (icx - 18, icy - 8, 36, 16), border_radius=6)
            pygame.draw.rect(surface, ic, (icx - 14, icy - 4, 28, 8), border_radius=3)
        else:
            pygame.draw.rect(surface, (30, 30, 35), (icx - 10, icy - 5, 20, 10), border_radius=4)
            pygame.draw.rect(surface, ic, (icx - 7, icy - 2, 14, 5), border_radius=2)
    elif shape == "pig_nose":
        # Rosa Rüsselscheibe mit zwei ovalen Nüstern
        if big:
            pygame.draw.ellipse(surface, ic, (icx - 13, icy - 9, 26, 18))
            pygame.draw.ellipse(surface, (190, 100, 120), (icx - 13, icy - 9, 26, 18), width=2)
            pygame.draw.ellipse(surface, (190, 100, 120), (icx - 7, icy - 3, 5, 7))
            pygame.draw.ellipse(surface, (190, 100, 120), (icx + 2, icy - 3, 5, 7))
        else:
            pygame.draw.ellipse(surface, ic, (icx - 8, icy - 5, 16, 11))
            pygame.draw.ellipse(surface, (190, 100, 120), (icx - 4, icy - 2, 3, 4))
            pygame.draw.ellipse(surface, (190, 100, 120), (icx + 1, icy - 2, 3, 4))
    elif shape == "bird_beak":
        # Gelber Vogelschnabel, Ober- und Unterschnabel
        if big:
            pygame.draw.polygon(surface, ic, [(icx - 14, icy - 5), (icx + 15, icy), (icx - 11, icy + 4)])
            pygame.draw.polygon(surface, (200, 140, 15), [(icx - 14, icy + 2), (icx + 10, icy + 5), (icx - 11, icy + 10)])
        else:
            pygame.draw.polygon(surface, ic, [(icx - 8, icy - 3), (icx + 9, icy), (icx - 6, icy + 2)])
            pygame.draw.polygon(surface, (200, 140, 15), [(icx - 8, icy + 1), (icx + 6, icy + 3), (icx - 6, icy + 6)])
    elif shape == "vampire_fangs":
        # Zwei spitze weisse Reisszähne
        if big:
            pygame.draw.polygon(surface, ic, [(icx - 9, icy - 6), (icx - 3, icy - 6), (icx - 6, icy + 8)])
            pygame.draw.polygon(surface, ic, [(icx + 3, icy - 6), (icx + 9, icy - 6), (icx + 6, icy + 8)])
        else:
            pygame.draw.polygon(surface, ic, [(icx - 5, icy - 4), (icx - 1, icy - 4), (icx - 3, icy + 5)])
            pygame.draw.polygon(surface, ic, [(icx + 1, icy - 4), (icx + 5, icy - 4), (icx + 3, icy + 5)])
    elif shape == "nerd_teeth":
        # Zwei breite, quadratische Hasenzähne mit Mittelspalt (Nerd-Look)
        if big:
            pygame.draw.line(surface, (10, 10, 15), (icx - 10, icy - 6), (icx + 10, icy - 6), 2)
            pygame.draw.rect(surface, ic, (icx - 9, icy - 6, 8, 12), border_radius=2)
            pygame.draw.rect(surface, ic, (icx + 1, icy - 6, 8, 12), border_radius=2)
            pygame.draw.rect(surface, (200, 200, 195), (icx - 9, icy - 6, 8, 12), width=1, border_radius=2)
            pygame.draw.rect(surface, (200, 200, 195), (icx + 1, icy - 6, 8, 12), width=1, border_radius=2)
        else:
            pygame.draw.line(surface, (10, 10, 15), (icx - 6, icy - 4), (icx + 6, icy - 4), 1)
            pygame.draw.rect(surface, ic, (icx - 5, icy - 4, 5, 7), border_radius=1)
            pygame.draw.rect(surface, ic, (icx + 1, icy - 4, 5, 7), border_radius=1)

# Alten, gemeinsamen 'equipped_player'-Slot (Kopfbedeckung + Gesicht teilten sich einen
# Slot) einmalig in die neuen, getrennten Kategorie-Slots migrieren - jetzt, wo
# COSMETICS_DB zur Kategorie-Bestimmung existiert.
if "equipped_player" in cosmetics_state:
    _migrate_cosmetics_equipped_player(cosmetics_state)
    save_section("cosmetics", cosmetics_state)
if "equipped_face" in cosmetics_state:
    _migrate_cosmetics_equipped_face(cosmetics_state)
    save_section("cosmetics", cosmetics_state)

def initialize_cosmetic_prices():
    """Zufällige Preise innerhalb des Bereichs beim ersten Laden festlegen und in cosmetics.json speichern."""
    # Stelle sicher, dass cosmetics_state ein 'prices' dict hat
    if "prices" not in cosmetics_state:
        cosmetics_state["prices"] = {}
    changed = False
    for key, meta in COSMETICS_DB.items():
        if key not in cosmetics_state.get("prices", {}):
            mn, mx = meta.get("price_range", [100, 300])  # Min/Max-Preis
            cosmetics_state.setdefault("prices", {})[key] = random.randint(int(mn), int(mx))
            changed = True
    if changed:
        save_section("cosmetics", cosmetics_state)  # Speichere feste Preise

def get_cosmetic_price(cosmetic_key):
    """Gibt den aktuellen Preis zurück (gespeicherter Preis, oder Mittelwert des
    Bereichs als Fallback), abzüglich eines eventuellen Sale-Rabatts von heute."""
    prices = cosmetics_state.get("prices", {})
    if cosmetic_key in prices:
        base_price = prices[cosmetic_key]  # Gespeicherter Preis
    else:
        meta = COSMETICS_DB.get(cosmetic_key, {})
        rng = meta.get("price_range", [100, 300])
        base_price = (rng[0] + rng[1]) // 2  # Mittelwert als Fallback
    discount = get_sale_discount(cosmetic_key)
    if discount:
        return max(1, round(base_price * (100 - discount) / 100))
    return base_price

def get_cosmetic_base_price(cosmetic_key):
    """Gibt den Preis OHNE Sale-Rabatt zurück - für die durchgestrichene
    Originalpreis-Anzeige bei reduzierten Items."""
    prices = cosmetics_state.get("prices", {})
    if cosmetic_key in prices:
        return prices[cosmetic_key]
    meta = COSMETICS_DB.get(cosmetic_key, {})
    rng = meta.get("price_range", [100, 300])
    return (rng[0] + rng[1]) // 2

# =============================
# Täglicher Shop-Sale
# =============================
# Jeden Tag werden ein paar zufällige Kosmetik-Items reduziert (ähnlich dem Daily
# Reward). Die Auswahl wird in gameplay.json (Abschnitt "sale") gespeichert und
# nur neu gewürfelt, wenn sich das (gecachte!) aktuelle Datum geändert hat -
# siehe get_cached_current_date() weiter oben für den 5-Sekunden-Cache.
_SALE_ITEM_COUNT = 4
_SALE_DISCOUNTS = [20, 25, 30, 40, 50]
# Je seltener ein Item, desto unwahrscheinlicher landet es im Sale - ein Mythic
# soll deutlich seltener reduziert werden als ein Common. Gewichte für die
# gewichtete Ziehung ohne Zurücklegen in _weighted_sample_without_replacement().
_SALE_RARITY_WEIGHTS = {"Common": 100, "Rare": 45, "Epic": 20, "Legendary": 8, "Mythic": 3}

def _weighted_sample_without_replacement(weighted_items, k):
    """Zieht bis zu k eindeutige Keys aus [(key, gewicht), ...], ohne Zurücklegen,
    proportional zum Gewicht - höheres Gewicht = höhere Ziehwahrscheinlichkeit."""
    pool = list(weighted_items)
    chosen = []
    for _ in range(min(k, len(pool))):
        total_weight = sum(w for _, w in pool)
        if total_weight <= 0:
            break
        roll = random.uniform(0, total_weight)
        running = 0
        for i, (key, weight) in enumerate(pool):
            running += weight
            if running >= roll:
                chosen.append(key)
                pool.pop(i)
                break
    return chosen

def ensure_daily_sale():
    """Stellt sicher, dass der Sale des heutigen Tages existiert; würfelt bei
    Tageswechsel neue Sale-Items. Günstig genug, um jeden Frame aufgerufen zu
    werden - das eigentliche Datum wird ja bereits gecacht abgefragt."""
    global sale_state
    current_date, has_internet = get_cached_current_date()
    if not has_internet:
        return  # Ohne Zeitabgleich lieber den alten Sale behalten als raten
    if sale_state.get("date") == current_date and sale_state.get("items"):
        return  # Sale ist schon aktuell
    weighted_keys = [
        (k, _SALE_RARITY_WEIGHTS.get(meta.get("rarity"), 50))
        for k, meta in COSMETICS_DB.items()
    ]
    count = min(_SALE_ITEM_COUNT, len(weighted_keys))
    chosen = _weighted_sample_without_replacement(weighted_keys, count)
    sale_state["date"] = current_date
    sale_state["items"] = {k: random.choice(_SALE_DISCOUNTS) for k in chosen}
    save_section("sale", sale_state)

def get_sale_discount(cosmetic_key):
    """Gibt den Rabatt (in %) für ein Item zurück, falls es heute im Angebot ist,
    sonst 0."""
    return sale_state.get("items", {}).get(cosmetic_key, 0)

def get_sale_items():
    """Gibt die Liste der Item-Keys zurück, die heute im Angebot sind."""
    return list(sale_state.get("items", {}).keys())

def buy_cosmetic(cosmetic_key):
    """Allgemeine Kauf-Logik für Kosmetik-Items."""
    owned = cosmetics_state.get("owned", [])
    if cosmetic_key in owned:
        # Lokalisierte "Bereits gekauft" Nachricht
        push_overlay(get_text("power_already_owned") if get_text("power_already_owned") else "Bereits gekauft", (200,120,120))
        return False
    price = get_cosmetic_price(cosmetic_key)
    if stats.get("coins", 0) < price:
        push_overlay(get_text("insufficient_coins"), (200,100,100))  # Nicht genug Coins
        return False
    # Kauf durchführen
    stats["coins"] = stats.get("coins", 0) - price
    cosmetics_state.setdefault("owned", []).append(cosmetic_key)
    save_section("stats", stats)
    save_section("cosmetics", cosmetics_state)
    # Lokalisierte Erfolgsnachricht anzeigen
    meta = COSMETICS_DB.get(cosmetic_key, {})
    name = meta.get("name", {}).get(languages[language_index], cosmetic_key)
    push_overlay(get_text("item_acquired").format(name), (255,215,0))
    return True

# Initialize cosmetic prices at startup
# (this will run after cosmetics_state is loaded below in the script flow - see usage)
# Berechnete maximale Slots (1 Basis + Extra Slot) → Extra Slot muss nicht ausgerüstet werden!
def get_max_slots():
    """Gibt die maximale Anzahl an Power-Slots zurück (1 Basis + 1 wenn Extra Slot besessen)."""
    return 2 if "Extra Slot" in powers_state.get("owned", []) else 1

# =============================
# UI Helpers (Benutzeroberflächen-Hilfsfunktionen)
# =============================

def get_text(key):
    """Gibt den lokalisierten Text für einen Schlüssel zurück.

    Sucht zuerst in der aktuellen Sprache, dann als Fallback in Deutsch,
    und schließlich den Schlüssel selbst wenn nichts gefunden wird.

    Args:
        key (str): Der Text-Schlüssel

    Returns:
        str: Der lokalisierte Text
    """
    lang = languages[language_index]  # Aktuelle Sprache
    pack = texts.get(lang, texts["Deutsch"])  # Sprachpaket oder Deutsch als Fallback
    return pack.get(key, texts["Deutsch"].get(key, key))  # Text oder Schlüssel


def get_difficulty_display_name(difficulty):
    """Übersetzt einen internen Schwierigkeits-Namen (immer Deutsch, z.B. aus
    'difficulties' oder von saved_scores/dem Online-Leaderboard) in die aktuelle
    UI-Sprache. Die internen Namen selbst bleiben überall Deutsch (Speicherdaten,
    Server-Keys) - das hier ist rein für die Anzeige."""
    return get_text(f"diff_{difficulty}")

def is_dark_mode():
    """Prüft, ob der Dark Mode aktiviert ist.

    Returns:
        bool: True wenn Dark Mode aktiv, False sonst
    """
    return settings.get("dark_mode", True)

def get_bg_color():
    """Gibt die Hintergrundfarbe basierend auf dem aktuellen Modus zurück.

    Returns:
        tuple: RGB-Farbwert für den Hintergrund
    """
    return DARK_BG if is_dark_mode() else (240, 245, 255)  # Hellblau im Light Mode

def get_panel_color():
    """Gibt die Panel-Farbe basierend auf dem aktuellen Modus zurück.

    Returns:
        tuple: RGB-Farbwert für Panels
    """
    return DARK_PANEL if is_dark_mode() else (255, 255, 255)  # Weiß im Light Mode

def get_text_color():
    """Gibt die Textfarbe basierend auf dem aktuellen Modus zurück.

    Returns:
        tuple: RGB-Farbwert für Text
    """
    return DARK_TEXT if is_dark_mode() else (30, 30, 60)  # Dunkelblau im Light Mode

def get_button_color():
    """Gibt die Button-Farbe basierend auf dem aktuellen Modus zurück.

    Returns:
        tuple: RGB-Farbwert für Buttons
    """
    return DARK_BUTTON if is_dark_mode() else (255, 255, 255)  # Weiß im Light Mode

def get_button_hover_color():
    """Gibt die Button-Hover-Farbe basierend auf dem aktuellen Modus zurück.

    Returns:
        tuple: RGB-Farbwert für Button-Hover-Effekte
    """
    return DARK_BUTTON_HOVER if is_dark_mode() else (200, 220, 255)  # Hellblau im Light Mode

def toggle_dark_mode():
    """Schaltet zwischen Dark Mode und Light Mode um.

    Speichert die Änderung und startet einen Loading Screen für den Übergang.
    """
    global settings
    settings["dark_mode"] = not settings.get("dark_mode", False)  # Umschalten
    save_section("settings", settings)  # Speichern
    # Loading Screen beim Wechseln des Dark Mode
    start_loading_screen(False)  # Zu Hauptmenü zurück
    play_sound(click_sound)  # Klick-Sound abspielen


def get_power_desc(power_name, detailed=False):
    """Gibt die Beschreibung einer Power in der aktuellen Sprache zurück.

    Args:
        power_name (str): Name der Power
        detailed (bool): True für detaillierte Beschreibung, False für kurze

    Returns:
        str: Die lokalisierte Beschreibung oder Standardtext
    """
    power_data = POWERS_DB.get(power_name, {})
    key = "detailed_desc" if detailed else "desc"  # Wähle Beschreibungstyp
    desc_data = power_data.get(key, power_data.get("desc", {}))

    # Falls es noch die alte String-Struktur ist, verwende sie direkt
    if isinstance(desc_data, str):
        return desc_data

    # Ansonsten verwende die mehrsprachige Struktur
    current_lang = languages[language_index]
    return desc_data.get(current_lang, desc_data.get("Deutsch", "Keine Beschreibung"))


def get_power_name(power_name):
    """Gibt den Namen einer Power in der aktuellen Sprache zurück.

    Args:
        power_name (str): Interner Name der Power

    Returns:
        str: Der lokalisierte Name oder der interne Name als Fallback
    """
    power_data = POWERS_DB.get(power_name, {})
    name_data = power_data.get("name", {})

    # Falls es noch die alte String-Struktur ist, verwende sie direkt
    if isinstance(name_data, str):
        return name_data

    # Ansonsten verwende die mehrsprachige Struktur
    current_lang = languages[language_index]
    return name_data.get(current_lang, name_data.get("English", power_name))


def get_cosmetic_name(cosmetic_key):
    """Gibt den lokalisierten Anzeigenamen eines Cosmetic-Items zurück (z.B.
    'fire_trail' -> 'Feuerspur'), Fallback auf den internen Key."""
    meta = COSMETICS_DB.get(cosmetic_key, {})
    return meta.get("name", {}).get(languages[language_index], cosmetic_key)


def get_achievement_name(code):
    """Gibt den Namen eines Achievements in der aktuellen Sprache zurück"""
    data = achievement_definitions.get(code, ({}, None, 0))
    name_data = data[0]
    if isinstance(name_data, dict):
        current_lang = languages[language_index]
        return name_data.get(current_lang, name_data.get("Deutsch", code))
    else:
        return name_data


class Button:
    def __init__(self, x, y, w, h, text, callback, font, color=GRAY, hover_color=DARK_GRAY, text_color=BLACK):
        self.rect = pygame.Rect(x, y, w, h)
        self.callback = callback
        self.font = font
        self.color = color
        self.hover_color = hover_color
        self.text = text
        self.text_color = text_color

    def draw(self, base_surface, mouse_pos):
        # Einfache Button-Zeichnung für andere Menüs
        color = self.hover_color if self.rect.collidepoint(mouse_pos) else self.color
        pygame.draw.rect(base_surface, color, self.rect, border_radius=12)
        text_surf = self.font.render(self.text, True, self.text_color)
        base_surface.blit(text_surf, (self.rect.centerx - text_surf.get_width() // 2,
                                self.rect.centery - text_surf.get_height() // 2))

    def handle_event(self, event, mouse_pos):
        if event.type == pygame.MOUSEBUTTONDOWN and self.rect.collidepoint(mouse_pos):
            self.callback()


# =============================
# Shared UI Drawing Helpers (Cards, Buttons, Glow-Pulse)
# Bisher hat praktisch jeder Screen Buttons/Panels einzeln von Hand gezeichnet
# (collidepoint -> Farbe wählen -> Rect -> Rahmen -> Text blitten, dutzendfach
# copy-pasted). Diese Helfer bündeln genau dieses Muster an einer Stelle, damit
# ein Stil-Update an vielen Screens gleichzeitig ankommt. Reine Zeichenfunktionen -
# Klick-/Hover-/Fokus-Erkennung bleibt beim Aufrufer, hier wird nichts angefasst
# was Spiellogik oder Eingabe betrifft.
# =============================

def ui_glow_pulse(base_color, speed=5.0, low=0.72, t=None):
    """Pulsierende Helligkeitsvariante einer Farbe (Sinus-Welle), wie bisher einzeln
    im Achievement-Popup/Banner-Code eingebaut. low = Mindesthelligkeit (0-1)."""
    if t is None:
        t = time.time()
    k = low + (1.0 - low) * (0.5 + 0.5 * math.sin(t * speed))
    return tuple(min(255, int(c * k)) for c in base_color)


def draw_card(surface, rect, radius=16, shadow=True, border_color=None, border_width=3,
              glow=False, glow_color=None):
    """Panel im Stil des Roll-Ergebnis-Popups: Schatten + Panel-Fläche + Rahmen,
    optional mit pulsierendem Glow-Rahmen (Achievement-Popup-Stil) statt festem
    Rahmen. Für Popups/Overlays/Hervorhebungen (z.B. Game Over, neuer Highscore)."""
    dark = is_dark_mode()
    if shadow:
        shadow_rect = pygame.Rect(rect.x + 5, rect.y + 5, rect.w, rect.h)
        pygame.draw.rect(surface, (0, 0, 0, 80) if dark else (170, 170, 190), shadow_rect, border_radius=radius)
    pygame.draw.rect(surface, get_panel_color(), rect, border_radius=radius)
    if glow:
        gc = glow_color or (255, 195, 50)
        pulse_col = ui_glow_pulse(gc, speed=5.0, low=0.7)
        outer = tuple(c // 4 for c in gc)
        pygame.draw.rect(surface, outer, rect.inflate(6, 6), width=2, border_radius=radius + 3)
        pygame.draw.rect(surface, pulse_col, rect, width=border_width, border_radius=radius)
    else:
        bc = border_color or ((70, 90, 110) if dark else (60, 80, 140))
        pygame.draw.rect(surface, bc, rect, width=border_width, border_radius=radius)


def draw_ui_button(surface, rect, text, font=None, hovered=False, base_color=None,
                    hover_color=None, text_color=None, border_color=None,
                    hover_border_color=None, radius=12, border_width=2, hover_border_width=3):
    """Standard-Button: Hintergrund + Rahmen + zentrierter Text - das Muster, das
    bisher an jedem Screen einzeln von Hand nachgebaut wurde. `hovered` deckt Maus-
    Hover UND Tastatur-/Controller-Fokus ab (vom Aufrufer kombiniert, z.B.
    `rect.collidepoint(mp) or nav_is_focused(...)`). Reine Zeichenfunktion -
    Klick-Erkennung bleibt wie bisher beim Aufrufer."""
    font = font or menu_font
    bg = (hover_color or get_button_hover_color()) if hovered else (base_color or get_button_color())
    pygame.draw.rect(surface, bg, rect, border_radius=radius)
    bc = (hover_border_color or (150, 180, 220)) if hovered else (border_color or (180, 180, 180))
    bw = hover_border_width if hovered else border_width
    pygame.draw.rect(surface, bc, rect, width=bw, border_radius=radius)
    txt_col = text_color or get_text_color()
    text_surf = font.render(text, True, txt_col)
    surface.blit(text_surf, (rect.centerx - text_surf.get_width() // 2,
                              rect.centery - text_surf.get_height() // 2))


# =============================
# Menu Buttons (Menü-Buttons)
# =============================

def switch_language():
    """Wechselt zur nächsten Sprache im Sprachen-Array.

    Zyklisch durch alle verfügbaren Sprachen (Deutsch, English, Русский, etc.).
    Speichert die neue Sprache in den Einstellungen und aktualisiert die Button-Texte.
    Spielt einen Klick-Sound ab.
    """
    global language_index
    language_index = (language_index + 1) % len(languages)
    settings["language"] = languages[language_index]
    save_section("settings", settings)
    update_button_texts()
    play_sound(click_sound)


def switch_difficulty():
    """Wechselt zur nächsten Schwierigkeitsstufe im Schwierigkeiten-Array.

    Zyklisch durch alle verfügbaren Schwierigkeiten (Leicht, Mittel, Schwer, etc.).
    Speichert die neue Schwierigkeit in den Einstellungen und aktualisiert die Button-Texte.
    Spielt einen Klick-Sound ab.
    """
    global difficulty_index
    difficulty_index = (difficulty_index + 1) % len(difficulties)
    settings["difficulty"] = difficulties[difficulty_index]
    save_section("settings", settings)
    update_button_texts()
    play_sound(click_sound)


def start_game():
    play_sound(menu_click_sound)
    global in_menu, in_difficulty_select, _ds_slider_edit
    in_menu = False
    in_difficulty_select = True
    _ds_slider_edit = None


def actually_start_game():
    play_sound(menu_click_sound)
    global klassik_mode
    klassik_mode = (current_game_mode == "klassik")
    def _do():
        global in_difficulty_select, game_running
        in_difficulty_select = False
        game_running = True
    request_fade(_do)


def show_statistics():
    global in_menu, showing_stats
    in_menu = False
    showing_stats = True


def open_shop_menu():
    play_sound(menu_click_sound)
    global in_menu, in_shop_menu
    in_menu = False
    in_shop_menu = True


def open_loadout_menu():
    play_sound(menu_click_sound)
    global in_menu, in_loadout_menu
    in_menu = False
    in_loadout_menu = True

def open_profile_hub():
    play_sound(menu_click_sound)
    global in_menu, in_profile_hub
    in_menu = False
    in_profile_hub = True

def back_to_main_from_profile_hub():
    play_sound(click_back_sound)
    global in_profile_hub, in_menu
    in_profile_hub = False
    in_menu = True

def open_profile_select_from_hub():
    play_sound(menu_click_sound)
    global in_profile_hub, in_profile_select
    in_profile_hub = False
    in_profile_select = True

def open_online_leaderboard_from_hub():
    play_sound(menu_click_sound)
    global in_profile_hub, in_online_leaderboard
    in_profile_hub = False
    in_online_leaderboard = True

def back_to_hub_from_online_leaderboard():
    play_sound(click_back_sound)
    global in_online_leaderboard, in_profile_hub
    in_online_leaderboard = False
    in_profile_hub = True

def open_player_search_from_leaderboard():
    play_sound(menu_click_sound)
    global in_online_leaderboard, in_player_search, _search_query, _search_results, _search_loading
    in_online_leaderboard = False
    in_player_search = True
    _search_query = ""
    _search_results = None
    _search_loading = False

def back_from_player_search():
    play_sound(click_back_sound)
    global in_player_search, in_online_leaderboard
    in_player_search = False
    in_online_leaderboard = True

def open_public_profile(name, return_screen):
    play_sound(menu_click_sound)
    global in_online_leaderboard, in_player_search, in_public_profile, in_friends_hub, in_clan_hub
    global _public_profile_name, _public_profile_data, _public_profile_loading, _public_profile_return_screen
    global _friend_action_pending, _public_profile_friend_msg
    in_online_leaderboard = False
    in_player_search = False
    in_friends_hub = False
    in_clan_hub = False
    in_public_profile = True
    _public_profile_name = name
    _public_profile_data = None
    _public_profile_loading = False
    _public_profile_return_screen = return_screen
    _friend_action_pending = None
    _public_profile_friend_msg = ""

def open_redeem_code_from_more():
    # Kein eigener play_sound() hier - die Aufrufstelle im "Mehr"-Menü
    # (in_more_menu-Dispatch) spielt für jede nicht-"back"-Aktion bereits
    # menu_click_sound, ein zweiter Aufruf hier würde ihn doppelt abspielen.
    global in_more_menu, in_redeem_code, _redeem_code_text, _redeem_code_msg, _redeem_code_busy
    in_more_menu = False
    in_redeem_code = True
    _redeem_code_text = ""
    _redeem_code_msg = ""
    _redeem_code_busy = False

def open_rename_profile_from_hub():
    play_sound(menu_click_sound)
    global in_profile_hub, in_rename_profile, _rename_text, _rename_msg, _rename_busy
    in_profile_hub = False
    in_rename_profile = True
    _rename_text = ""
    _rename_msg = ""
    _rename_busy = False

def back_from_public_profile():
    play_sound(click_back_sound)
    global in_public_profile, in_online_leaderboard, in_player_search, in_friends_hub, in_clan_hub
    in_public_profile = False
    if _public_profile_return_screen == "in_player_search":
        in_player_search = True
    elif _public_profile_return_screen == "in_friends_hub":
        in_friends_hub = True
    elif _public_profile_return_screen == "in_clan_hub":
        in_clan_hub = True
    else:
        in_online_leaderboard = True

def get_daily_reward_status():
    """Gibt den Status des Daily Rewards zurück.

    Überprüft, ob die tägliche Belohnung verfügbar ist und gibt den entsprechenden
    lokalisierten Status-Text zurück (verfügbar, bereits erhalten, kein Internet).

    Returns:
        str: Lokalisierter Status-Text für die Daily Reward Anzeige
    """
    can_claim, message = can_claim_daily_reward()
    if not can_claim:
        if "Internet" in message:
            return get_text("daily_reward_no_internet")
        else:
            return get_text("daily_reward_claimed")
    return get_text("daily_reward_available")

def get_time_until_reset():
    """Berechnet die Zeit bis zum nächsten täglichen Reset (Daily Reward & Sale).

    Nutzt die gecachte Uhrzeit (siehe get_cached_now), damit hier nicht bei jedem
    Aufruf eine echte Internet-/Zeitanfrage gestellt wird - die Sekunden zählen
    trotzdem jeden Frame flüssig weiter, da get_cached_now() zwischen den echten
    Checks lokal weiterrechnet.

    Returns:
        str: Formatierte Zeit (HH:MM:SS) oder Fallback-Text
    """
    try:
        current_time, has_internet = get_cached_now()
        if not has_internet:
            return "Keine Internetverbindung"

        # Nächster Reset ist um 00:00 Uhr des nächsten Tages
        next_reset = current_time.replace(hour=0, minute=0, second=0, microsecond=0) + timedelta(days=1)
        time_diff = next_reset - current_time

        hours = int(time_diff.total_seconds() // 3600)
        minutes = int((time_diff.total_seconds() % 3600) // 60)
        seconds = int(time_diff.total_seconds() % 60)

        return f"{hours:02d}:{minutes:02d}:{seconds:02d}"
    except:
        return "Unbekannt"

def claim_daily_reward_from_menu():
    """Vergibt die tägliche Belohnung aus dem Menü heraus.

    Ruft die Belohnungsvergabe-Funktion auf und aktualisiert die Button-Texte,
    um die neuen Coin-Anzahlen anzuzeigen.

    Returns:
        bool: True bei erfolgreicher Vergabe, False sonst
    """
    success = claim_daily_reward()
    if success:
        # Button-Texts aktualisieren, um die neuen Coins anzuzeigen
        update_button_texts()
    return success

def draw_daily_reward_panel():
    """Zeichnet das Daily Reward Panel im Hauptmenü.

    Zeigt den aktuellen Status der täglichen Belohnung, die letzte Belohnung
    mit Details (Coins, Rarity, Bonus) und die Reset-Zeit an.
    """
    # Panel-Hintergrund
    panel_rect = pygame.Rect(WIDTH - 320, 60, 300, 200)
    pygame.draw.rect(base_surface, (245, 248, 255), panel_rect, border_radius=12)
    pygame.draw.rect(base_surface, (60, 80, 140), panel_rect, width=2, border_radius=12)

    # Titel
    title = small_font.render(get_text("daily_reward"), True, (30, 30, 60))
    base_surface.blit(title, (panel_rect.x + 15, panel_rect.y + 15))

    # Status
    can_claim, message = can_claim_daily_reward()
    if can_claim:
        status_text = small_font.render(get_text('daily_reward_available'), True, (0, 150, 0))
    elif "Internet" in message:
        status_text = small_font.render(get_text('daily_reward_no_internet'), True, (200, 100, 100))
    else:
        status_text = small_font.render(get_text('daily_reward_claimed'), True, (150, 150, 150))

    base_surface.blit(status_text, (panel_rect.x + 15, panel_rect.y + 40))

    # Letzte Belohnung anzeigen
    last_stats = stats.get("last_daily_reward_stats", {})
    if last_stats:
        y_offset = 70

        # Letzte Belohnung Titel
        last_title = small_font.render(f"{get_text('last_reward')}:", True, (80, 80, 80))
        base_surface.blit(last_title, (panel_rect.x + 15, panel_rect.y + y_offset))
        y_offset += 25

        # Coins
        coins_text = small_font.render(f"{last_stats.get('total', 0)} {get_text('coins')}", True, (255, 215, 0))
        base_surface.blit(coins_text, (panel_rect.x + 15, panel_rect.y + y_offset))
        y_offset += 20

        # Rarity
        rarity = last_stats.get('rarity', 'Normal')
        rarity_colors = {"Normal": (130, 130, 140), "Rare": (80, 150, 255), "Epic": (180, 80, 255)}
        rarity_color = rarity_colors.get(rarity, (130, 130, 140))
        rarity_name = get_text(f"rarity_{rarity.lower()}")
        rarity_text = small_font.render(f"{get_text('rarity_label')}: {rarity_name}", True, rarity_color)
        base_surface.blit(rarity_text, (panel_rect.x + 15, panel_rect.y + y_offset))
        y_offset += 20

        extra = last_stats.get('extra', 0)
        if extra > 0:
            extra_col = (180, 80, 255) if rarity == "Epic" else (80, 150, 255)
            extra_text = small_font.render(f"+{extra} {get_text('coins')} Extra", True, extra_col)
            base_surface.blit(extra_text, (panel_rect.x + 15, panel_rect.y + y_offset))

    # Reset-Zeit Info
    reset_text = small_font.render(get_text('reset_time'), True, (120, 120, 120))
    base_surface.blit(reset_text, (panel_rect.x + 15, panel_rect.bottom - 25))

def start_loading_screen(to_daily_rewards=True):
    """Startet den Loading Screen mit Übergangseffekt.

    Args:
        to_daily_rewards (bool): True für Übergang zu Daily Rewards, False für Hauptmenü
    """
    global in_menu, in_loading, loading_progress, loading_start_time, loading_to_daily_rewards
    in_menu = False
    in_loading = True
    loading_progress = 0
    loading_start_time = pygame.time.get_ticks()
    loading_to_daily_rewards = to_daily_rewards

def open_daily_rewards_menu():
    """Öffnet das Daily Rewards Menü mit Loading Screen.

    Spielt einen Menü-Klick-Sound und startet den Loading Screen für den Übergang.
    """
    play_sound(menu_click_sound)
    start_loading_screen(True)

def handle_daily_reward():
    """Behandelt den Daily Reward Button - öffnet jetzt das Menü.

    Delegiert an open_daily_rewards_menu() für konsistente Handhabung.
    """
    open_daily_rewards_menu()

# Admin: open login/panel

def open_admin_login():
    """Öffnet das Admin-Login-Fenster.

    Wechselt vom Hauptmenü zum Admin-Login-Modus und initialisiert die Eingabefelder.
    Spielt einen Menü-Klick-Sound ab.
    """
    global in_menu, in_admin_login, admin_pwd_input, admin_msg
    in_menu = False
    in_admin_login = True
    admin_pwd_input = ""
    admin_msg = ""
    play_sound(menu_click_sound)

# =============================
# Menu Functions (moved before buttons creation)
# =============================

def open_more_menu():
    play_sound(menu_click_sound)
    set_all_menu_flags(False)
    global in_more_menu
    in_more_menu = True

def open_settings_menu():
    play_sound(menu_click_sound)
    set_all_menu_flags(False)
    global in_settings_menu
    in_settings_menu = True

def update_button_texts():
    """Aktualisiert die Texte aller sichtbaren Buttons im Hauptmenü.

    Passt die Button-Texte an die aktuelle Sprache und Schwierigkeit an.
    Zeigt die aktuelle Coin-Anzahl im Shop-Button an.
    Behandelt sowohl alte als auch neue Button-Layouts für Kompatibilität.
    """
    # Update visible button labels (main menu + shop coin display)
    local_diff = difficulties[difficulty_index]
    # main buttons mapping: language switch moved into 'More' submenu
    # buttons order: [LanguagePlaceholder, Difficulty, Start, StatsPlaceholder, Shop, Loadout, AdminPlaceholder]
    # We will replace with new mapping for fewer buttons
    # Rebuild buttons' text safely depending on how many buttons currently exist
    try:
        # If old layout still present (7 buttons), update compatibly
        if len(buttons) >= 7:
            buttons[0].text = get_text("language")
            buttons[1].text = f"{get_text('difficulty')}: {get_difficulty_name(local_diff)}"
            buttons[2].text = get_text("start")
            buttons[3].text = get_text("stats")
            buttons[4].text = get_text("shop")
            buttons[5].text = get_text("loadout")
            buttons[6].text = get_text("admin")
        else:
            # New layout: [Start, Shop, Loadout, Settings, More]
            if len(buttons) >= 1: buttons[0].text = get_text("start")
            if len(buttons) >= 2: buttons[1].text = get_text("shop")
            if len(buttons) >= 3: buttons[2].text = get_text("loadout")
            if len(buttons) >= 4: buttons[3].text = get_text("settings")
            if len(buttons) >= 5: buttons[4].text = get_text("more")
    except Exception as e:
        print('[update_button_texts] Fehler:', e)

def _get_profile_hub_btn_rect():
    """Rect des Profil-Buttons unten links im Hauptmenü - HEIGHT-abhängig (oberhalb des
    Mods-Buttons, falls der grad sichtbar ist), deshalb als Funktion statt fixem Rect und
    sowohl im Event-Handler als auch in draw_menu() separat aufgerufen (gleiches Muster
    wie mods_btn_check/mods_btn_rect). Unten links statt oben links (war vorher direkt
    unter der Coin-Anzeige) - dort ist immer genug Freiraum nach oben für hohe
    Kopfbedeckungen (Propeller-Mütze etc.), ohne mit der Coin-Anzeige zu kollidieren."""
    return pygame.Rect(15, HEIGHT - 118, 220, 56)

# Erstelle die Hauptmenü-Buttons mit ihren Positionen, Größen und Callbacks
buttons = []
start_x = WIDTH//2 - 180  # Breiter für längere Texte
start_y = 155
gap = 56
buttons.append(Button(start_x, start_y + 0*gap, 360, 48, get_text('start'), start_game, menu_font))
buttons.append(Button(start_x, start_y + 1*gap, 360, 48, get_text('shop'), open_shop_menu, menu_font))
buttons.append(Button(start_x, start_y + 2*gap, 360, 48, get_text('loadout'), open_loadout_menu, menu_font))
buttons.append(Button(start_x, start_y + 3*gap, 360, 48, get_text('settings'), open_settings_menu, menu_font))
buttons.append(Button(start_x, start_y + 4*gap, 360, 48, get_text('more'), lambda: open_more_menu(), menu_font))
def draw_menu():
    mouse_pos = get_mouse_pos_logical()

    # Play menu music if not playing
    if sound_enabled and menu_music_path and not pygame.mixer.music.get_busy():
        play_menu_music()

    # Buttons jedes Frame neu zentrieren (damit Resize korrekt funktioniert)
    bx = WIDTH // 2 - 180
    for i, btn in enumerate(buttons):
        btn.rect.x = bx
        btn.rect.y = 155 + i * 56

    _menu_has_mods  = len(mod_system.mods) > 0
    _menu_nav_total = len(buttons) + (1 if _menu_has_mods else 0) + 2  # + Mods-Button (bedingt) + Daily-Reward + Profil-Button

    # Schöner Gradient-Hintergrund
    fill_bg(rings=True)

    # Haupttitel mit Schatten-Effekt
    title = font.render("JumpNDodge", True, get_text_color())
    title_shadow = font.render("JumpNDodge", True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH // 2 - title.get_width() // 2 + 3, 73))
    base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 70))
    
    # Untertitel
    subtitle = small_font.render(get_text("game_subtitle"), True, (80, 80, 100))
    base_surface.blit(subtitle, (WIDTH // 2 - subtitle.get_width() // 2, 110))
    
    # Coin-Anzeige - selber Stil wie auf jedem anderen Screen (Shop, Mehr-Menü, etc.:
    # get_panel_color()/get_text_color(), Dark-Mode-tauglich). Hauptmenü war bisher die
    # einzige Ausnahme mit hart codiertem Weiß, das im Dark Mode nicht mitgewechselt hat.
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)

    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))

    # Profil-Button (Avatar-Vorschau + Name) - unten links, Avatar in echter Gameplay-
    # Größe (50x50), damit Kosmetik genauso aussieht wie im Spiel selbst.
    _profile_hub_btn_rect = _get_profile_hub_btn_rect()
    _profile_nav_idx = len(buttons) + (1 if _menu_has_mods else 0) + 1  # nach Mods (bedingt) + Daily-Reward
    _profile_hovered = _profile_hub_btn_rect.collidepoint(mouse_pos) or nav_is_focused("menu", _profile_nav_idx, _menu_nav_total)
    draw_ui_button(base_surface, _profile_hub_btn_rect, "", hovered=_profile_hovered, radius=10)
    _pb_avatar_rect = pygame.Rect(_profile_hub_btn_rect.x + 3, _profile_hub_btn_rect.y + 3, 50, 50)
    draw_avatar(base_surface, _pb_avatar_rect)
    _pb_name_surf = small_font.render(current_profile_name or "-", True, get_text_color())
    _pb_name_surf = _fit_surf(_pb_name_surf, _profile_hub_btn_rect.width - _pb_avatar_rect.width - 20)
    base_surface.blit(_pb_name_surf, (_pb_avatar_rect.right + 10,
                                       _profile_hub_btn_rect.centery - _pb_name_surf.get_height() // 2))

    # Buttons mit verbessertem Design
    for i, button in enumerate(buttons):
        is_hovered = button.rect.collidepoint(mouse_pos) or nav_is_focused("menu", i, _menu_nav_total)
        draw_ui_button(base_surface, button.rect, button.text, font=button.font, hovered=is_hovered)
    
    # Mods Button (only shown when at least one mod is loaded)
    if _menu_has_mods:
        _mc = mod_system.get_active_count()
        _mb_label = f"Mods ({_mc})" if _mc > 0 else "Mods"
        mods_btn_rect = pygame.Rect(20, HEIGHT - 56, 130, 36)
        _mb_hovered   = mods_btn_rect.collidepoint(mouse_pos) or nav_is_focused("menu", len(buttons), _menu_nav_total)
        draw_ui_button(base_surface, mods_btn_rect, _mb_label, font=small_font, hovered=_mb_hovered,
                        base_color=(60, 40, 160), hover_color=(80, 60, 200), text_color=(255, 255, 255),
                        border_color=(100, 80, 200), hover_border_color=(180, 160, 255))

    # Daily Rewards Button in der oberen rechten Ecke
    daily_reward_rect = pygame.Rect(WIDTH - 200, 15, 180, 40)
    _daily_idx = len(buttons) + (1 if _menu_has_mods else 0)
    is_hovered = daily_reward_rect.collidepoint(mouse_pos) or nav_is_focused("menu", _daily_idx, _menu_nav_total)
    draw_ui_button(base_surface, daily_reward_rect, get_text("daily_rewards"), font=small_font, hovered=is_hovered,
                    base_color=(255, 200, 0), hover_color=(255, 215, 0), text_color=(0, 0, 0),
                    border_color=(180, 120, 0), hover_border_color=(200, 150, 0), radius=8)

    pygame.display.flip()


# =============================
# Mods Menu
# =============================

def open_mods_menu():
    global in_menu, in_mods_menu, _mods_scroll, _mods_edit, _mods_edit_text
    play_sound(menu_click_sound)
    in_menu      = False
    in_mods_menu = True
    _mods_scroll = 0
    _mods_edit   = None
    _mods_edit_text = ''


def close_mods_menu():
    global in_menu, in_mods_menu
    play_sound(click_back_sound)
    in_mods_menu = False
    in_menu      = True


def draw_mods_menu():
    """Render the full Mods menu with per-mod settings UI."""
    global _mods_scroll, _mods_edit, _mods_edit_text
    fill_bg(rings=True)
    mouse_pos = get_mouse_pos_logical()
    dark = is_dark_mode()

    # ---- Title ----
    title_surf = font.render("MODS", True, get_text_color())
    base_surface.blit(title_surf,
                      (WIDTH // 2 - title_surf.get_width() // 2, 12))

    count_surf = small_font.render(
        f"{mod_system.get_active_count()} / {len(mod_system.mods)} active",
        True, (255, 180, 0)
    )
    base_surface.blit(count_surf,
                      (WIDTH // 2 - count_surf.get_width() // 2, 56))

    # ---- Back button ----
    back_rect = pygame.Rect(20, 18, 100, 36)
    back_col  = (80, 60, 180) if back_rect.collidepoint(mouse_pos) else (60, 40, 140)
    pygame.draw.rect(base_surface, back_col, back_rect, border_radius=10)
    back_surf = small_font.render("< Back", True, (255, 255, 255))
    base_surface.blit(back_surf, (back_rect.centerx - back_surf.get_width() // 2,
                                   back_rect.centery - back_surf.get_height() // 2))

    # ---- No mods fallback ----
    if not mod_system.mods:
        empty_surf = menu_font.render(
            'No mods found in mods/ folder.', True, (160, 160, 160)
        )
        base_surface.blit(empty_surf,
                          (WIDTH // 2 - empty_surf.get_width() // 2,
                           HEIGHT // 2 - empty_surf.get_height() // 2))
        pygame.display.flip()
        return back_rect, []

    # ---- Scrollable mod cards ----
    CARD_X      = 30
    CARD_W      = WIDTH - 60
    CARD_MARGIN = 14
    content_top = 82
    visible_h   = HEIGHT - content_top - 12
    clip_rect   = pygame.Rect(CARD_X, content_top, CARD_W, visible_h)

    # Clipping surface so cards don't render outside scroll area
    clip_surf = pygame.Surface((CARD_W, visible_h), pygame.SRCALPHA)

    # Collect rects that callers need for click-hit-testing (absolute coords)
    hit_rects = []   # list of (action, data, absolute_rect)

    y = -_mods_scroll
    for mod_idx, mod in enumerate(mod_system.mods):
        # --- Calculate card height dynamically ---
        is_broken    = mod.error is not None
        setting_rows = 0 if is_broken else len(mod.settings)
        card_h       = 100 if is_broken else (120 + setting_rows * 38)

        # --- Card background ---
        card_rect = pygame.Rect(0, y, CARD_W, card_h)
        if card_rect.bottom < 0 or card_rect.top > visible_h:
            # Off-screen Karten werden nicht in hit_rects aufgenommen -> per Tastatur aktuell
            # nicht erreichbar, bis reingescrollt wurde (Mausrad funktioniert weiterhin).
            y += card_h + CARD_MARGIN
            continue

        is_broken = mod.error is not None
        if is_broken:
            bg_col  = (70, 20, 20) if dark else (255, 230, 230)
            brd_col = (220, 60, 60)
        else:
            bg_col  = (45, 45, 70) if dark else (240, 240, 255)
            brd_col = (255, 180, 0) if mod.active else (120, 120, 140)
        pygame.draw.rect(clip_surf, bg_col,  card_rect, border_radius=12)
        pygame.draw.rect(clip_surf, brd_col, card_rect, width=2, border_radius=12)

        # --- Mod name + api badge ---
        name_col  = (255, 100, 100) if is_broken else get_text_color()
        name_surf = menu_font.render(mod.name, True, name_col)
        clip_surf.blit(name_surf, (14, y + 10))


        # --- Error banner (shown instead of description when broken) ---
        if is_broken:
            err_label = small_font.render("! LOAD ERROR", True, (220, 60, 60))
            clip_surf.blit(err_label, (14, y + 38))
            # Show full error message, wrapping at ~90 chars
            err_text = mod.error
            err_line1 = err_text[:90]
            err_line2 = err_text[90:180] if len(err_text) > 90 else ''
            el1 = small_font.render(err_line1, True, (200, 100, 100) if dark else (160, 40, 40))
            clip_surf.blit(el1, (14, y + 58))
            if err_line2:
                el2 = small_font.render(err_line2, True, (200, 100, 100) if dark else (160, 40, 40))
                clip_surf.blit(el2, (14, y + 76))
        else:
            # --- Description ---
            desc_surf = small_font.render(mod.description[:80], True,
                                          (180, 180, 200) if dark else (80, 80, 100))
            clip_surf.blit(desc_surf, (14, y + 38))

            # --- Version / Author ---
            meta_str  = f"v{mod.version}  by {mod.author}"
            meta_surf = small_font.render(meta_str, True,
                                          (140, 140, 160) if dark else (110, 110, 130))
            clip_surf.blit(meta_surf, (14, y + 58))

        # --- Enable toggle (grayed out / locked for broken mods) ----
        tog_rect_rel = pygame.Rect(CARD_W - 110, y + 10, 96, 26)
        if is_broken:
            tog_col  = (100, 60, 60)
            tog_text = "BROKEN"
        else:
            tog_col  = (60, 200, 80) if mod.active else (180, 60, 60)
            tog_text = "ENABLED" if mod.active else "DISABLED"
        pygame.draw.rect(clip_surf, tog_col, tog_rect_rel, border_radius=8)
        tog_surf = small_font.render(tog_text, True, (255, 255, 255))
        clip_surf.blit(tog_surf, (tog_rect_rel.centerx - tog_surf.get_width() // 2,
                                   tog_rect_rel.centery - tog_surf.get_height() // 2))
        if not is_broken:
            abs_tog = pygame.Rect(CARD_X + tog_rect_rel.x,
                                   content_top + tog_rect_rel.y - _mods_scroll,
                                   tog_rect_rel.w, tog_rect_rel.h)
            hit_rects.append(('toggle_mod', mod_idx, abs_tog))

        # --- Settings (skipped for broken mods) ---
        sy = y + 86
        for skey, sval in ([] if is_broken else mod.settings.items()):
            row_rect_rel = pygame.Rect(14, sy, CARD_W - 28, 30)
            row_label    = small_font.render(f"{skey}:", True, get_text_color())
            clip_surf.blit(row_label, (row_rect_rel.x, sy + 6))

            ctrl_x = row_rect_rel.x + 180
            ctrl_w = CARD_W - 28 - 180

            if isinstance(sval, bool):
                # Toggle
                tog_r_rel = pygame.Rect(ctrl_x, sy + 4, 60, 22)
                tc = (60, 200, 80) if sval else (160, 60, 60)
                pygame.draw.rect(clip_surf, tc, tog_r_rel, border_radius=6)
                tl = small_font.render("ON" if sval else "OFF", True, (255, 255, 255))
                clip_surf.blit(tl, (tog_r_rel.centerx - tl.get_width() // 2,
                                     tog_r_rel.centery - tl.get_height() // 2))
                abs_r = pygame.Rect(CARD_X + tog_r_rel.x,
                                    content_top + tog_r_rel.y - _mods_scroll,
                                    tog_r_rel.w, tog_r_rel.h)
                hit_rects.append(('toggle_setting', (mod_idx, skey), abs_r))

            elif isinstance(sval, (int, float)):
                # Slider: [-] value [+]
                btn_w = 26
                minus_r = pygame.Rect(ctrl_x, sy + 4, btn_w, 22)
                plus_r  = pygame.Rect(ctrl_x + btn_w + 60, sy + 4, btn_w, 22)
                val_r   = pygame.Rect(ctrl_x + btn_w + 2, sy + 4, 58, 22)

                pygame.draw.rect(clip_surf, (80, 80, 160), minus_r, border_radius=4)
                pygame.draw.rect(clip_surf, (80, 80, 160), plus_r,  border_radius=4)
                pygame.draw.rect(clip_surf, (50, 50, 90),  val_r,   border_radius=4)

                ml = small_font.render("-", True, (255, 255, 255))
                pl = small_font.render("+", True, (255, 255, 255))
                vl = small_font.render(str(sval), True, (220, 220, 220))
                clip_surf.blit(ml, (minus_r.centerx - ml.get_width() // 2,
                                     minus_r.centery - ml.get_height() // 2))
                clip_surf.blit(pl, (plus_r.centerx  - pl.get_width() // 2,
                                     plus_r.centery  - pl.get_height() // 2))
                clip_surf.blit(vl, (val_r.centerx   - vl.get_width() // 2,
                                     val_r.centery   - vl.get_height() // 2))

                def _abs(r_rel):
                    return pygame.Rect(CARD_X + r_rel.x,
                                       content_top + r_rel.y - _mods_scroll,
                                       r_rel.w, r_rel.h)
                hit_rects.append(('num_minus', (mod_idx, skey), _abs(minus_r)))
                hit_rects.append(('num_plus',  (mod_idx, skey), _abs(plus_r)))

            else:
                # Text input
                is_editing = (_mods_edit == (mod_idx, skey))
                disp_val   = _mods_edit_text if is_editing else str(sval)
                inp_r      = pygame.Rect(ctrl_x, sy + 4, min(ctrl_w, 160), 22)
                inp_col    = (255, 255, 200) if is_editing else (200, 200, 220)
                pygame.draw.rect(clip_surf, inp_col, inp_r, border_radius=4)
                pygame.draw.rect(clip_surf, (100, 100, 180), inp_r, width=1, border_radius=4)
                il = small_font.render(disp_val[:20], True, (30, 30, 30))
                clip_surf.blit(il, (inp_r.x + 4, inp_r.centery - il.get_height() // 2))
                abs_r = pygame.Rect(CARD_X + inp_r.x,
                                    content_top + inp_r.y - _mods_scroll,
                                    inp_r.w, inp_r.h)
                hit_rects.append(('input_focus', (mod_idx, skey), abs_r))

            sy += 38

        y += card_h + CARD_MARGIN

    total_h = y + _mods_scroll
    base_surface.blit(clip_surf, (CARD_X, content_top))

    # Tastatur-Fokus-Rahmen (einheitlicher Overlay-Pass für alle Controls + Back;
    # nur für aktuell sichtbare/registrierte hit_rects, siehe Kommentar bei "continue" oben)
    _mods_nav_total = len(hit_rects) + 1
    for _hi, (_ha, _hd, _hr) in enumerate(hit_rects):
        if nav_is_focused("mods_menu", _hi, _mods_nav_total):
            pygame.draw.rect(base_surface, (255, 255, 255), _hr, width=2, border_radius=4)
    if nav_is_focused("mods_menu", len(hit_rects), _mods_nav_total):
        pygame.draw.rect(base_surface, (255, 255, 255), back_rect, width=2, border_radius=10)

    # ---- Scrollbar ----
    if total_h > visible_h:
        sb_x    = WIDTH - 10
        sb_h    = max(30, int(visible_h * visible_h / total_h))
        sb_y    = content_top + int(_mods_scroll / total_h * visible_h)
        pygame.draw.rect(base_surface, (120, 120, 160),
                         pygame.Rect(sb_x, sb_y, 6, sb_h), border_radius=3)

    pygame.display.flip()
    return back_rect, hit_rects


def _save_mod_settings():
    """Persist every mod's current settings + active state into modded_data.json."""
    data = {}
    for mod in mod_system.mods:
        data[mod.name] = {
            'active':   mod.active,
            'settings': mod.settings,
        }
    save_modded_section('mod_settings', data)

def _load_mod_settings():
    """Restore mod settings + active state from modded_data.json (if it exists)."""
    if not os.path.isfile(_MODDED_DATA_FILE):
        return
    data = load_modded_section('mod_settings', {})
    if not data:
        return
    try:
        for mod in mod_system.mods:
            saved = data.get(mod.name)
            if not saved:
                continue
            if mod.error is None:          # never re-enable a broken mod
                mod.active = saved.get('active', mod.active)
            for k, v in saved.get('settings', {}).items():
                if k in mod.settings:      # only restore known keys
                    mod.settings[k] = v
    except Exception as exc:
        print(f'[MODS] Could not load settings: {exc}')


def _mods_handle_click(action, data):
    """Process a click (or keyboard-confirm) in the mods menu and save afterwards."""
    global _mods_edit, _mods_edit_text

    if action == 'toggle_mod':
        mod_system.mods[data].active = not mod_system.mods[data].active
        play_sound(click_sound)
        _save_mod_settings()

    elif action == 'toggle_setting':
        mod_idx, skey = data
        mod = mod_system.mods[mod_idx]
        mod.settings[skey] = not mod.settings[skey]
        play_sound(click_sound)
        _save_mod_settings()

    elif action == 'num_minus':
        mod_idx, skey = data
        mod = mod_system.mods[mod_idx]
        v = mod.settings[skey]
        step = 1 if isinstance(v, int) else 0.1
        mod.settings[skey] = round(v - step, 6)
        play_sound(click_sound)
        _save_mod_settings()

    elif action == 'num_plus':
        mod_idx, skey = data
        mod = mod_system.mods[mod_idx]
        v = mod.settings[skey]
        step = 1 if isinstance(v, int) else 0.1
        mod.settings[skey] = round(v + step, 6)
        play_sound(click_sound)
        _save_mod_settings()

    elif action == 'input_focus':
        mod_idx, skey = data
        if _mods_edit == (mod_idx, skey):
            # Confirm and save when deselecting the field
            mod_system.mods[mod_idx].settings[skey] = _mods_edit_text
            _mods_edit      = None
            _mods_edit_text = ''
            _save_mod_settings()
        else:
            _mods_edit      = (mod_idx, skey)
            _mods_edit_text = str(mod_system.mods[mod_idx].settings.get(skey, ''))
        play_sound(click_sound)


# =============================
# Difficulty Select Screen
# =============================

_diff_colors = {
    "Leicht":    (50,  200,  50),
    "Mittel":    (200, 200,  50),
    "Schwer":    (255, 140,   0),
    "Hardcore":  (255,  80,   0),
    "Unmöglich": (150,   0, 150),
    "Albtraum":  (139,   0,   0),
    "Hölle":     (20,  20,  20),
    "Dynamisch": (150,  80, 255),
}

_diff_short_translations = {
    "Deutsch": {
        "Leicht":    "Ideal für Einsteiger. Langsam & entspannt.",
        "Mittel":    "Ausgewogen. Guter Einstieg für alle.",
        "Schwer":    "Echte Herausforderung für Fortgeschrittene.",
        "Hardcore":  "Nur für Erfahrene. Hohe Geschwindigkeit.",
        "Unmöglich": "Extreme Geschwindigkeit. Kaum zu schaffen.",
        "Albtraum":  "Für die Härtesten. Fast nicht überlebbar.",
        "Hölle":     "Überleben ist fast unmöglich.",
        "Dynamisch": "Startet langsam – wird endlos schneller.",
    },
    "English": {
        "Leicht":    "Ideal for beginners. Slow & relaxed.",
        "Mittel":    "Balanced. A good start for everyone.",
        "Schwer":    "A real challenge for advanced players.",
        "Hardcore":  "Experienced players only. High speed.",
        "Unmöglich": "Extreme speed. Barely doable.",
        "Albtraum":  "For the toughest. Almost unsurvivable.",
        "Hölle":     "Survival is nearly impossible.",
        "Dynamisch": "Starts slow — gets endlessly faster.",
    },
    "Русский": {
        "Leicht":    "Идеально для новичков. Медленно и спокойно.",
        "Mittel":    "Сбалансировано. Хороший старт для всех.",
        "Schwer":    "Настоящий вызов для продвинутых игроков.",
        "Hardcore":  "Только для опытных. Высокая скорость.",
        "Unmöglich": "Экстремальная скорость. Едва проходимо.",
        "Albtraum":  "Для самых стойких. Почти невозможно выжить.",
        "Hölle":     "Выжить почти невозможно.",
        "Dynamisch": "Начинается медленно — становится бесконечно быстрее.",
    },
    "Türkçe": {
        "Leicht":    "Yeni başlayanlar için ideal. Yavaş ve rahat.",
        "Mittel":    "Dengeli. Herkes için iyi bir başlangıç.",
        "Schwer":    "İleri seviye oyuncular için gerçek bir meydan okuma.",
        "Hardcore":  "Sadece deneyimliler için. Yüksek hız.",
        "Unmöglich": "Aşırı hız. Zar zor başarılabilir.",
        "Albtraum":  "En güçlüler için. Neredeyse hayatta kalınamaz.",
        "Hölle":     "Hayatta kalmak neredeyse imkansız.",
        "Dynamisch": "Yavaş başlar — sonsuza dek hızlanır.",
    },
    "Schwiizerdütsch": {
        "Leicht":    "Ideal für Aafänger. Langsam & entspannt.",
        "Mittel":    "Usgwoge. Guete Iistig für alli.",
        "Schwer":    "Echti Useforderig für Fortgschrittni.",
        "Hardcore":  "Nur für Erfahreni. Hoi Gschwindigkeit.",
        "Unmöglich": "Extremi Gschwindigkeit. Chuum z schaffe.",
        "Albtraum":  "Für die Härtiste. Fasch nöd überlebbar.",
        "Hölle":     "Überläbe isch fasch unmögli.",
        "Dynamisch": "Startet langsam – wird ändlos schneller.",
    },
}

def get_diff_short(diff):
    """Gibt die lokalisierte Kurzbeschreibung einer Schwierigkeit zurück."""
    lang = languages[language_index]
    pack = _diff_short_translations.get(lang, _diff_short_translations["Deutsch"])
    return pack.get(diff, _diff_short_translations["Deutsch"].get(diff, diff))

_diff_hover_translations = {
    "Deutsch": {
        "Leicht":    ("Geschwindigkeit: 6  |  Hindernis-Pause: 1.4 – 2.2 s",
                      "Keine Coins. Ideal zum Lernen und Üben."),
        "Mittel":    ("Geschwindigkeit: 8  |  Hindernis-Pause: 1.0 – 1.8 s",
                      "Keine Coins. Gutes Training für höhere Schwierigkeiten."),
        "Schwer":    ("Geschwindigkeit: 11  |  Hindernis-Pause: 0.6 – 1.3 s",
                      "Coins: 10 Punkte = 1 Coin  (min. 15 Sek. überleben)"),
        "Hardcore":  ("Geschwindigkeit: 15  |  Hindernis-Pause: 0.4 – 1.0 s",
                      "Coins: 5 Punkte = 1 Coin  (min. 15 Sek. überleben)"),
        "Unmöglich": ("Geschwindigkeit: 18  |  Hindernis-Pause: 0.5 – 1.0 s",
                      "Coins: 3 Punkte = 1 Coin  (min. 15 Sek. überleben)"),
        "Albtraum":  ("Geschwindigkeit: 24  |  Hindernis-Pause: 0.2 – 0.5 s",
                      "Coins: 2 Punkte = 1 Coin  (min. 15 Sek. überleben)"),
        "Hölle":     ("Geschwindigkeit: 32  |  Hindernis-Pause: 0.12 – 0.3 s",
                      "Coins: 1,5 Punkte = 1 Coin (aufgerundet)  (min. 15 Sek. überleben)"),
        "Dynamisch": ("Startet bei Geschwindigkeit 7, Pause 1.4 – 2.4 s — steigt alle 5 Sek. für immer.",
                      "Keine Coins. Hört niemals auf schneller zu werden. Wie weit kommst du?"),
    },
    "English": {
        "Leicht":    ("Speed: 6  |  Obstacle Pause: 1.4 – 2.2 s",
                      "No coins. Ideal for learning and practicing."),
        "Mittel":    ("Speed: 8  |  Obstacle Pause: 1.0 – 1.8 s",
                      "No coins. Good training for higher difficulties."),
        "Schwer":    ("Speed: 11  |  Obstacle Pause: 0.6 – 1.3 s",
                      "Coins: 10 points = 1 coin  (survive min. 15 sec.)"),
        "Hardcore":  ("Speed: 15  |  Obstacle Pause: 0.4 – 1.0 s",
                      "Coins: 5 points = 1 coin  (survive min. 15 sec.)"),
        "Unmöglich": ("Speed: 18  |  Obstacle Pause: 0.5 – 1.0 s",
                      "Coins: 3 points = 1 coin  (survive min. 15 sec.)"),
        "Albtraum":  ("Speed: 24  |  Obstacle Pause: 0.2 – 0.5 s",
                      "Coins: 2 points = 1 coin  (survive min. 15 sec.)"),
        "Hölle":     ("Speed: 32  |  Obstacle Pause: 0.12 – 0.3 s",
                      "Coins: 1.5 points = 1 coin (rounded up)  (survive min. 15 sec.)"),
        "Dynamisch": ("Starts at speed 7, pause 1.4 – 2.4 s — increases every 5 sec. forever.",
                      "No coins. Never stops getting faster. How far will you get?"),
    },
    "Русский": {
        "Leicht":    ("Скорость: 6  |  Пауза препятствий: 1.4 – 2.2 с",
                      "Без монет. Идеально для изучения и практики."),
        "Mittel":    ("Скорость: 8  |  Пауза препятствий: 1.0 – 1.8 с",
                      "Без монет. Хорошая тренировка для высоких сложностей."),
        "Schwer":    ("Скорость: 11  |  Пауза препятствий: 0.6 – 1.3 с",
                      "Монеты: 10 очков = 1 монета  (мин. 15 сек. выжить)"),
        "Hardcore":  ("Скорость: 15  |  Пауза препятствий: 0.4 – 1.0 с",
                      "Монеты: 5 очков = 1 монета  (мин. 15 сек. выжить)"),
        "Unmöglich": ("Скорость: 18  |  Пауза препятствий: 0.5 – 1.0 с",
                      "Монеты: 3 очка = 1 монета  (мин. 15 сек. выжить)"),
        "Albtraum":  ("Скорость: 24  |  Пауза препятствий: 0.2 – 0.5 с",
                      "Монеты: 2 очка = 1 монета  (мин. 15 сек. выжить)"),
        "Hölle":     ("Скорость: 32  |  Пауза препятствий: 0.12 – 0.3 с",
                      "Монеты: 1,5 очка = 1 монета (округляется)  (мин. 15 сек. выжить)"),
        "Dynamisch": ("Начинается со скорости 7, пауза 1.4 – 2.4 с — увеличивается каждые 5 сек. бесконечно.",
                      "Без монет. Никогда не перестаёт ускоряться. Как далеко ты продвинешься?"),
    },
    "Türkçe": {
        "Leicht":    ("Hız: 6  |  Engel Arası: 1.4 – 2.2 sn",
                      "Coin yok. Öğrenmek ve alıştırma için ideal."),
        "Mittel":    ("Hız: 8  |  Engel Arası: 1.0 – 1.8 sn",
                      "Coin yok. Daha yüksek zorluklar için iyi antrenman."),
        "Schwer":    ("Hız: 11  |  Engel Arası: 0.6 – 1.3 sn",
                      "Coin: 10 puan = 1 coin  (min. 15 sn. hayatta kal)"),
        "Hardcore":  ("Hız: 15  |  Engel Arası: 0.4 – 1.0 sn",
                      "Coin: 5 puan = 1 coin  (min. 15 sn. hayatta kal)"),
        "Unmöglich": ("Hız: 18  |  Engel Arası: 0.5 – 1.0 sn",
                      "Coin: 3 puan = 1 coin  (min. 15 sn. hayatta kal)"),
        "Albtraum":  ("Hız: 24  |  Engel Arası: 0.2 – 0.5 sn",
                      "Coin: 2 puan = 1 coin  (min. 15 sn. hayatta kal)"),
        "Hölle":     ("Hız: 32  |  Engel Arası: 0.12 – 0.3 sn",
                      "Coin: 1,5 puan = 1 coin (yukarı yuvarlanır)  (min. 15 sn. hayatta kal)"),
        "Dynamisch": ("Hız 7'de başlar, ara 1.4 – 2.4 sn — her 5 sn.'de sonsuza dek artar.",
                      "Coin yok. Hızlanmayı asla bırakmaz. Ne kadar ileri gidebilirsin?"),
    },
    "Schwiizerdütsch": {
        "Leicht":    ("Gschwindigkeit: 6  |  Hindernis-Pause: 1.4 – 2.2 s",
                      "Kei Coins. Ideal zum Lehrne und Übe."),
        "Mittel":    ("Gschwindigkeit: 8  |  Hindernis-Pause: 1.0 – 1.8 s",
                      "Kei Coins. Guets Training für höchi Schwierigkeite."),
        "Schwer":    ("Gschwindigkeit: 11  |  Hindernis-Pause: 0.6 – 1.3 s",
                      "Coins: 10 Punkt = 1 Coin  (min. 15 Sek. überläbe)"),
        "Hardcore":  ("Gschwindigkeit: 15  |  Hindernis-Pause: 0.4 – 1.0 s",
                      "Coins: 5 Punkt = 1 Coin  (min. 15 Sek. überläbe)"),
        "Unmöglich": ("Gschwindigkeit: 18  |  Hindernis-Pause: 0.5 – 1.0 s",
                      "Coins: 3 Punkt = 1 Coin  (min. 15 Sek. überläbe)"),
        "Albtraum":  ("Gschwindigkeit: 24  |  Hindernis-Pause: 0.2 – 0.5 s",
                      "Coins: 2 Punkt = 1 Coin  (min. 15 Sek. überläbe)"),
        "Hölle":     ("Gschwindigkeit: 32  |  Hindernis-Pause: 0.12 – 0.3 s",
                      "Coins: 1,5 Punkt = 1 Coin (ufgrundet)  (min. 15 Sek. überläbe)"),
        "Dynamisch": ("Startet bi Gschwindigkeit 7, Pause 1.4 – 2.4 s — steigt alli 5 Sek. für immer.",
                      "Kei Coins. Hört nie uf schneller z werde. Wie wiit chunsch du?"),
    },
}

def get_diff_hover(diff):
    """Gibt das lokalisierte Hover-Tooltip (2 Zeilen) einer Schwierigkeit zurück."""
    lang = languages[language_index]
    pack = _diff_hover_translations.get(lang, _diff_hover_translations["Deutsch"])
    return pack.get(diff, _diff_hover_translations["Deutsch"].get(diff, ("", "")))

_modifier_hover_translations = {
    "Deutsch": {
        "custom_oneups": ("Steuert, ob während der Runde 1-UP-Powerups (extra Leben) spawnen können.",
                           "Aus = keine Extra-Leben mehr einsammelbar, sonst unverändert."),
        "custom_vertical_obstacles": ("Zusätzlich zu normalen Hindernissen fallen welche von oben herab.",
                                       "Macht die Runde spürbar schwerer, unabhängig von Speed/Pause-Slidern."),
        "custom_wind_sides": ("Böiger Wind von links/rechts schiebt Spieler und Boden-Hindernisse horizontal.",
                               "Stärke & Richtung wechseln automatisch in Phasen - mal Flaute, mal starke Böe."),
        "custom_wind_all": ("Wie Wind (Seiten), plus vertikaler Wind von oben/unten als zweite Achse.",
                             "Wind von oben bremst deinen Fall (man wird getragen), Wind von unten beschleunigt ihn."),
        "custom_free_roam": ("Ersetzt Sprung/Schwerkraft durch freie WASD-Bewegung in alle 4 Richtungen.",
                              "Hindernisse kommen dann von allen 4 Bildschirmkanten statt nur von rechts."),
    },
    "English": {
        "custom_oneups": ("Controls whether 1-UP powerups (extra lives) can spawn during the round.",
                           "Off = no more extra lives to collect, otherwise unchanged."),
        "custom_vertical_obstacles": ("In addition to normal obstacles, some fall from above.",
                                       "Makes the round noticeably harder, independent of the speed/pause sliders."),
        "custom_wind_sides": ("Gusty wind from left/right pushes the player and ground obstacles horizontally.",
                               "Strength & direction change automatically in phases - sometimes calm, sometimes a strong gust."),
        "custom_wind_all": ("Like Wind (Sides), plus vertical wind from above/below as a second axis.",
                             "Wind from above slows your fall (you get carried), wind from below speeds it up."),
        "custom_free_roam": ("Replaces jump/gravity with free WASD movement in all 4 directions.",
                              "Obstacles then come from all 4 screen edges instead of just the right."),
    },
    "Русский": {
        "custom_oneups": ("Определяет, могут ли во время раунда появляться усиления 1-UP (доп. жизни).",
                           "Выкл. = больше нельзя собрать доп. жизни, в остальном без изменений."),
        "custom_vertical_obstacles": ("В дополнение к обычным препятствиям некоторые падают сверху.",
                                       "Заметно усложняет раунд, независимо от ползунков скорости/паузы."),
        "custom_wind_sides": ("Порывистый ветер слева/справа толкает игрока и наземные препятствия по горизонтали.",
                               "Сила и направление меняются автоматически фазами - то штиль, то сильный порыв."),
        "custom_wind_all": ("Как Ветер (по бокам), плюс вертикальный ветер сверху/снизу как вторая ось.",
                             "Ветер сверху замедляет падение (тебя несёт), ветер снизу ускоряет его."),
        "custom_free_roam": ("Заменяет прыжок/гравитацию свободным движением WASD во всех 4 направлениях.",
                              "Препятствия тогда появляются со всех 4 краёв экрана, а не только справа."),
    },
    "Türkçe": {
        "custom_oneups": ("Tur sırasında 1-UP güçlendirmelerinin (ekstra can) belirip belirmeyeceğini kontrol eder.",
                           "Kapalı = artık ekstra can toplanamaz, geri kalanı değişmez."),
        "custom_vertical_obstacles": ("Normal engellere ek olarak bazıları yukarıdan düşer.",
                                       "Hız/ara kaydırıcılarından bağımsız olarak turu belirgin şekilde zorlaştırır."),
        "custom_wind_sides": ("Sol/sağdan gelen sağanak rüzgar oyuncuyu ve yerdeki engelleri yatay itiyor.",
                               "Güç ve yön otomatik olarak evreler halinde değişir - bazen sakin, bazen güçlü esinti."),
        "custom_wind_all": ("Rüzgar (Yanlar) gibi, artı ikinci eksen olarak yukarıdan/aşağıdan dikey rüzgar.",
                             "Yukarıdan gelen rüzgar düşüşünü yavaşlatır (taşınırsın), aşağıdan geleni hızlandırır."),
        "custom_free_roam": ("Zıplama/yer çekimini 4 yönde serbest WASD hareketiyle değiştirir.",
                              "Engeller o zaman sadece sağdan değil, ekranın 4 kenarından da gelir."),
    },
    "Schwiizerdütsch": {
        "custom_oneups": ("Stürt, öb während de Runde 1-UP-Powerups (extra Läbe) chöi spawne.",
                           "Us = kei extra Läbe me sammelbar, sonst unverändert."),
        "custom_vertical_obstacles": ("Zuesätzlich zu normale Hindernis falled welligi vo obe abe.",
                                       "Macht d Runde spürbar schwerer, unabhängig vo Speed/Pause-Slider."),
        "custom_wind_sides": ("Böige Wind vo links/rechts schiebt Spieler und Bode-Hindernis horizontal.",
                               "Stärki & Richtig wechsled automatisch i Phase - mal Flouti, mal starchi Böö."),
        "custom_wind_all": ("Wie Wind (Site), plus vertikale Wind vo obe/une als zwöiti Achse.",
                             "Wind vo obe bremst dis Falle (me wird treit), Wind vo une beschleunigt's."),
        "custom_free_roam": ("Ersetzt Sprung/Schwerkraft dur frei WASD-Bewegig i alli 4 Richtige.",
                              "Hindernis chömed denn vo allne 4 Bildschirmchante statt nur vo rechts."),
    },
}

def get_modifier_hover(key):
    """Gibt das lokalisierte Hover-Tooltip (2 Zeilen) für einen Custom-Mode-Modifier zurück."""
    lang = languages[language_index]
    pack = _modifier_hover_translations.get(lang, _modifier_hover_translations["Deutsch"])
    return pack.get(key, _modifier_hover_translations["Deutsch"].get(key, ("", "")))


def _fit_surf(surf, max_w):
    """Scale a surface down if it exceeds max_w, preserving aspect ratio."""
    if surf.get_width() <= max_w:
        return surf
    scale = max_w / surf.get_width()
    return pygame.transform.smoothscale(surf, (max_w, max(1, int(surf.get_height() * scale))))


def draw_difficulty_select():
    mouse_pos = get_mouse_pos_logical()

    dark = is_dark_mode()

    # Fokus-Reihenfolge: Sidebar-Tabs (+Co-op/VS falls 2-Player) -> Hauptbereich (Slider+Toggles/Karten) -> Start, Back
    _ds_sidebar_count = 4 + (2 if current_game_mode == "2player" else 0)
    _ds_main_count = 8 if current_game_mode == "custom" else len(difficulties)  # 3 Slider + 5 Toggle
    _ds_total = _ds_sidebar_count + _ds_main_count + 2
    _ds_start_idx = _ds_sidebar_count + _ds_main_count
    _ds_back_idx = _ds_start_idx + 1

    # Hintergrund: Menü-BG weiterhin sichtbar
    fill_bg(rings=True)

    # Dunkler Overlay über dem Hintergrund
    _dim = pygame.Surface((WIDTH, HEIGHT), pygame.SRCALPHA)
    _dim.fill((0, 0, 0, 150))
    base_surface.blit(_dim, (0, 0))


    # Title
    title_surf = menu_font.render(get_text("choose_difficulty_title"), True, get_text_color())
    base_surface.blit(title_surf, (WIDTH // 2 - title_surf.get_width() // 2, 30))

    # ---- Sidebar: Custom ist der Hauptmodus (großer Button), Presets (Standard/Klassik/
    # 2 Player) sind kompakt daneben - Custom bekommt dadurch den ganzen Hauptbereich statt
    # sich einen gleichwertigen Tab mit den Presets zu teilen. ----
    SIDEBAR_X, SIDEBAR_W = 24, 180
    MAIN_X = SIDEBAR_X + SIDEBAR_W + 24
    MAIN_W = max(300, WIDTH - MAIN_X - 24)
    MAIN_Y0 = 118

    _klassik_accent = (180, 140, 0)    # gold für Klassik
    _2p_accent      = (140, 60, 200)   # lila für 2-Player
    _custom_accent  = (0, 160, 170)    # türkis für Custom

    custom_btn_rect = pygame.Rect(SIDEBAR_X, MAIN_Y0, SIDEBAR_W, 56)
    _c_active = current_game_mode == "custom"
    _c_bg = _custom_accent if _c_active else ((35, 35, 50) if dark else (200, 205, 220))
    _c_brd = (100, 230, 230) if _c_active else ((80, 80, 100) if dark else (150, 155, 175))
    _c_tc = (255, 255, 255) if _c_active else ((160, 160, 180) if dark else (80, 85, 110))
    _c_focused = nav_is_focused("difficulty_select", 0, _ds_total)
    if (custom_btn_rect.collidepoint(mouse_pos) or _c_focused) and not _c_active:
        _c_bg = (0, 70, 75) if dark else (100, 190, 195)
    pygame.draw.rect(base_surface, _c_bg, custom_btn_rect, border_radius=12)
    pygame.draw.rect(base_surface, (255, 255, 255) if _c_focused else _c_brd, custom_btn_rect, width=3, border_radius=12)
    _c_lbl = menu_font.render("Custom", True, _c_tc)
    base_surface.blit(_c_lbl, (custom_btn_rect.centerx - _c_lbl.get_width() // 2, custom_btn_rect.top + 8))
    _c_sub = small_font.render("Slider & Modifier", True, _c_tc)
    base_surface.blit(_c_sub, (custom_btn_rect.centerx - _c_sub.get_width() // 2, custom_btn_rect.bottom - 22))

    _presets_lbl_y = custom_btn_rect.bottom + 20
    _presets_lbl = small_font.render("PRESETS", True, (140, 140, 160) if dark else (120, 120, 140))
    base_surface.blit(_presets_lbl, (SIDEBAR_X, _presets_lbl_y))

    preset_btn_y = _presets_lbl_y + 22
    standard_btn_rect = pygame.Rect(SIDEBAR_X, preset_btn_y, SIDEBAR_W, 42)
    klassik_btn_rect  = pygame.Rect(SIDEBAR_X, standard_btn_rect.bottom + 8, SIDEBAR_W, 42)
    tp_btn_rect       = pygame.Rect(SIDEBAR_X, klassik_btn_rect.bottom + 8, SIDEBAR_W, 42)

    for _pi, (prect, label, is_active, accent) in enumerate([
        (standard_btn_rect, "Standard", current_game_mode == "standard", None),
        (klassik_btn_rect,  "Klassik",  current_game_mode == "klassik",  _klassik_accent),
        (tp_btn_rect,       "2 Player", current_game_mode == "2player",  _2p_accent),
    ]):
        _p_focused = nav_is_focused("difficulty_select", 1 + _pi, _ds_total)
        if is_active:
            p_bg  = accent if accent else ((50, 80, 160) if dark else (60, 100, 200))
            p_brd = (255, 220, 80) if accent == _klassik_accent else \
                    ((200, 140, 255) if accent else ((120, 160, 255) if dark else (40, 70, 180)))
            p_tc  = (255, 255, 255)
        else:
            p_bg  = (35, 35, 50) if dark else (200, 205, 220)
            p_brd = (80, 80, 100) if dark else (150, 155, 175)
            p_tc  = (160, 160, 180) if dark else (80, 85, 110)
        if (prect.collidepoint(mouse_pos) or _p_focused) and not is_active:
            if accent == _klassik_accent:
                p_bg = (100, 75, 0) if dark else (200, 165, 40)
            elif accent == _2p_accent:
                p_bg = (60, 35, 80) if dark else (160, 120, 210)
            else:
                p_bg = (45, 55, 80) if dark else (180, 185, 210)
        pygame.draw.rect(base_surface, p_bg,  prect, border_radius=10)
        pygame.draw.rect(base_surface, (255, 255, 255) if _p_focused else p_brd, prect, width=2, border_radius=10)
        lsurf = small_font.render(label, True, p_tc)
        base_surface.blit(lsurf, (prect.centerx - lsurf.get_width() // 2,
                                  prect.centery - lsurf.get_height() // 2))

    # Co-op / VS Sub-Buttons (nur wenn 2 Player aktiv), unter dem 2-Player-Button
    if current_game_mode == "2player":
        sub_w = (SIDEBAR_W - 8) // 2
        sub_y = tp_btn_rect.bottom + 10
        tab_coop_rect = pygame.Rect(SIDEBAR_X, sub_y, sub_w, 32)
        tab_vs_rect   = pygame.Rect(SIDEBAR_X + sub_w + 8, sub_y, sub_w, 32)
        for _si, (srect, slabel, s_active) in enumerate([
            (tab_coop_rect, "Co-op", two_player_submode == "coop"),
            (tab_vs_rect,   "VS",    two_player_submode == "vs"),
        ]):
            _s_focused = nav_is_focused("difficulty_select", 4 + _si, _ds_total)
            if s_active:
                s_bg  = (100, 40, 160) if dark else (120, 50, 190)
                s_brd = (200, 140, 255)
                s_tc  = (255, 255, 255)
            else:
                s_bg  = (35, 35, 50) if dark else (210, 210, 225)
                s_brd = (80, 80, 100) if dark else (150, 150, 175)
                s_tc  = (150, 150, 170) if dark else (70, 70, 100)
            if (srect.collidepoint(mouse_pos) or _s_focused) and not s_active:
                s_bg = (55, 30, 80) if dark else (185, 185, 210)
            pygame.draw.rect(base_surface, s_bg,  srect, border_radius=8)
            pygame.draw.rect(base_surface, (255, 255, 255) if _s_focused else s_brd, srect, width=2, border_radius=8)
            ss = small_font.render(slabel, True, s_tc)
            base_surface.blit(ss, (srect.centerx - ss.get_width() // 2,
                                   srect.centery - ss.get_height() // 2))

    # ---- Hauptbereich (rechts neben der Sidebar) ----
    card_w = MAIN_W
    card_x = MAIN_X
    cards_y0 = MAIN_Y0
    card_h = 50
    gap = 56

    hovered_diff = None
    hovered_diff_rect = None

    if current_game_mode == "custom":
        # ---- Custom-Modus: Slider für Geschwindigkeit & Hindernis-Pause, Modifier-Toggles ----
        custom_speed     = settings.get("custom_speed", 10)
        custom_min_delay = settings.get("custom_min_delay", 900)
        custom_max_delay = settings.get("custom_max_delay", 1600)

        _slider_labels = ["Geschwindigkeit:", "Min-Pause:", "Max-Pause:"]
        _label_w = max(menu_font.size(l)[0] for l in _slider_labels)
        slider_x = card_x + _label_w + 24
        slider_w = card_w - (_label_w + 24) - 80

        for si, (label, value, lo, hi, y, value_txt) in enumerate([
            ("Geschwindigkeit:", custom_speed,     4,   35,   cards_y0 + 30,  str(int(custom_speed))),
            ("Min-Pause:",       custom_min_delay, 100, 2000, cards_y0 + 100, f"{custom_min_delay/1000:.2f}s"),
            ("Max-Pause:",       custom_max_delay, 200, 2500, cards_y0 + 170, f"{custom_max_delay/1000:.2f}s"),
        ]):
            _sl_focused = nav_is_focused("difficulty_select", _ds_sidebar_count + si, _ds_total)
            _sl_editing = (_ds_slider_edit == si)
            row_rect = pygame.Rect(card_x, y - 20, card_w, 40)
            if _sl_focused:
                pygame.draw.rect(base_surface, (255, 255, 255), row_rect, width=2, border_radius=8)
            track_rect = pygame.Rect(slider_x, y - 4, slider_w, 8)
            pygame.draw.rect(base_surface, get_panel_color(), track_rect, border_radius=4)
            pygame.draw.rect(base_surface, (255, 210, 60) if _sl_editing else (150, 180, 220), track_rect, width=2, border_radius=4)
            frac = max(0.0, min(1.0, (value - lo) / (hi - lo)))
            handle_x = int(slider_x + frac * slider_w)
            handle_r = 13 if _sl_editing else 10
            pygame.draw.circle(base_surface, (255, 200, 40) if _sl_editing else (0, 160, 170), (handle_x, y), handle_r)
            pygame.draw.circle(base_surface, (255, 230, 150) if _sl_editing else (150, 230, 230), (handle_x, y), handle_r, 2)
            lbl_surf = menu_font.render(label, True, get_text_color())
            base_surface.blit(lbl_surf, (card_x, y - lbl_surf.get_height() // 2))
            val_surf = menu_font.render(value_txt, True, get_text_color())
            base_surface.blit(val_surf, (card_x + card_w - val_surf.get_width(), y - val_surf.get_height() // 2))

        # Modifiers
        mod_y = cards_y0 + 210
        mod_lbl = menu_font.render("Modifier:", True, get_text_color())
        base_surface.blit(mod_lbl, (card_x, mod_y))

        toggle_defs = [
            ("custom_oneups", True, "1-Ups"),
            ("custom_vertical_obstacles", False, "Vert. Hindernisse"),
            ("custom_wind_sides", False, "Wind (Seiten)"),
            ("custom_wind_all", False, "Wind (Allseitig)"),
            ("custom_free_roam", False, "Freie Bewegung (Beta)"),
        ]
        tw, th, tgap, row_gap, cols = 220, 34, 12, 10, 2
        toggle_y = mod_y + 34
        _toggle_rows = -(-len(toggle_defs) // cols)  # aufgerundete Division
        hovered_modifier = None
        hovered_modifier_rect = None
        for ti, (skey, default, tlabel) in enumerate(toggle_defs):
            active = settings.get(skey, default)
            col, row = ti % cols, ti // cols
            tr = pygame.Rect(card_x + col * (tw + tgap), toggle_y + row * (th + row_gap), tw, th)
            _t_focused = nav_is_focused("difficulty_select", _ds_sidebar_count + 3 + ti, _ds_total)
            _t_mouse_hover = tr.collidepoint(mouse_pos)
            is_mod_hovered = _t_mouse_hover or _t_focused
            if is_mod_hovered:
                hovered_modifier = skey
                hovered_modifier_rect = None if _t_mouse_hover else tr
            bg_col = (0, 140, 150) if active else get_button_color()
            border_col = (255, 255, 255) if _t_focused else \
                         ((120, 240, 240) if (is_mod_hovered and not active) else \
                         ((100, 220, 220) if active else (160, 160, 160)))
            pygame.draw.rect(base_surface, bg_col, tr, border_radius=8)
            pygame.draw.rect(base_surface, border_col, tr, width=(3 if is_mod_hovered else 2), border_radius=8)
            ts = small_font.render(tlabel, True, (255, 255, 255) if active else get_text_color())
            base_surface.blit(ts, (tr.centerx - ts.get_width() // 2, tr.centery - ts.get_height() // 2))

        # Genereller Hinweis (immer sichtbar); die Modifier-Erklärung selbst kommt jetzt als
        # Cursor-Tooltip statt hier fix eingeblendet zu werden.
        _hint_y = toggle_y + _toggle_rows * (th + row_gap) + 4
        hint = small_font.render("Keine Coins im Custom-Modus. Freie Bewegung: WASD.", True, (150, 150, 150) if dark else (110, 110, 110))
        base_surface.blit(hint, (card_x, _hint_y))
        if hovered_modifier and hovered_modifier in _modifier_hover_translations["Deutsch"]:
            draw_hover_tooltip(get_modifier_hover(hovered_modifier), mouse_pos, max_width=600, anchor_rect=hovered_modifier_rect)
    else:
        for i, diff in enumerate(difficulties):
            card_y = cards_y0 + i * gap
            card_rect = pygame.Rect(card_x, card_y, card_w, card_h)
            _diff_mouse_hover = card_rect.collidepoint(mouse_pos)
            is_hovered = _diff_mouse_hover or nav_is_focused("difficulty_select", _ds_sidebar_count + i, _ds_total)
            is_selected = (i == difficulty_index)
            col = _diff_colors[diff]
            _col_bright = sum(col) > 60  # Farbe hell genug für sichtbaren Rahmen?
            _border_col = col if _col_bright else ((200, 200, 200) if dark else (80, 80, 80))

            if is_hovered:
                hovered_diff = diff
                hovered_diff_rect = None if _diff_mouse_hover else card_rect

            # Card background — dark-mode aware selection highlight
            if is_selected:
                if dark:
                    sel_bg = (max(0, col[0]//5), max(0, col[1]//5), max(0, col[2]//5))
                    pygame.draw.rect(base_surface, sel_bg, card_rect, border_radius=12)
                    pygame.draw.rect(base_surface, _border_col, card_rect, width=3, border_radius=12)
                else:
                    pygame.draw.rect(base_surface, (220, 240, 255), card_rect, border_radius=12)
                    pygame.draw.rect(base_surface, _border_col, card_rect, width=3, border_radius=12)
            elif is_hovered:
                pygame.draw.rect(base_surface, get_button_hover_color(), card_rect, border_radius=12)
                pygame.draw.rect(base_surface, _border_col, card_rect, width=2, border_radius=12)
            else:
                pygame.draw.rect(base_surface, get_button_color(), card_rect, border_radius=12)
                pygame.draw.rect(base_surface, (130, 130, 130) if dark else (180, 180, 180),
                                 card_rect, width=1, border_radius=12)

            # Colored left indicator strip
            pygame.draw.rect(base_surface, col, pygame.Rect(card_x, card_y, 7, card_h), border_radius=4)

            # Difficulty name
            _sel_text_col = (210, 210, 210) if not _col_bright else col
            name_col = _sel_text_col if is_selected else get_text_color()
            name_surf = menu_font.render(get_difficulty_name(diff), True, name_col)
            base_surface.blit(name_surf, (card_x + 18, card_y + card_h // 2 - name_surf.get_height() // 2))

            # Short description (only if space allows)
            desc_col = get_text_color() if (is_hovered or is_selected) else ((150, 150, 150) if dark else (110, 110, 110))
            desc_surf = small_font.render(get_diff_short(diff), True, desc_col)
            base_surface.blit(desc_surf, (card_x + 175, card_y + card_h // 2 - desc_surf.get_height() // 2))

            # Right badge
            if is_selected:
                badge_col = _sel_text_col if dark else (0, 110, 190)
                badge = small_font.render(get_text("selected_badge"), True, badge_col)
            elif klassik_mode:
                badge = small_font.render(get_text("no_coins_badge"), True, (120, 120, 120))
            elif diff in coin_rules:
                badge = small_font.render(get_text("pts_per_coin").format(coin_rules[diff]), True, (200, 155, 0) if dark else (160, 115, 0))
            else:
                badge = small_font.render(get_text("no_coins_badge"), True, (120, 120, 120))
            base_surface.blit(badge, (card_rect.right - badge.get_width() - 14,
                                      card_y + card_h // 2 - badge.get_height() // 2))

        # Hover tooltip - direkt neben dem Cursor (Maus) bzw. neben der Karte (Tastatur)
        if hovered_diff:
            draw_hover_tooltip(get_diff_hover(hovered_diff), mouse_pos, max_width=600, accent_color=_diff_colors[hovered_diff], anchor_rect=hovered_diff_rect)

    # Start + Back buttons at bottom, side by side
    btn_y = HEIGHT - 54
    btn_h = 44
    start_rect = pygame.Rect(WIDTH // 2 + 8,  btn_y, 200, btn_h)
    back_rect  = pygame.Rect(WIDTH // 2 - 208, btn_y, 200, btn_h)

    # Start button (accent color, sanfter Puls als Blickfang für die primäre Aktion)
    _start_focused = nav_is_focused("difficulty_select", _ds_start_idx, _ds_total)
    start_hov = start_rect.collidepoint(mouse_pos) or _start_focused
    start_bg = (0, 140, 60) if start_hov else (0, 110, 50)
    start_border = (255, 255, 255) if _start_focused else ui_glow_pulse((0, 200, 80), speed=3.0, low=0.65)
    pygame.draw.rect(base_surface, start_bg, start_rect, border_radius=10)
    pygame.draw.rect(base_surface, start_border, start_rect, width=2, border_radius=10)
    start_lbl = menu_font.render(get_text("start"), True, (255, 255, 255))
    base_surface.blit(start_lbl, (start_rect.centerx - start_lbl.get_width() // 2,
                                  start_rect.centery - start_lbl.get_height() // 2))

    # Back button
    _back_focused = nav_is_focused("difficulty_select", _ds_back_idx, _ds_total)
    back_hov = back_rect.collidepoint(mouse_pos) or _back_focused
    pygame.draw.rect(base_surface,
                     get_button_hover_color() if back_hov else get_button_color(),
                     back_rect, border_radius=10)
    pygame.draw.rect(base_surface, (255, 255, 255) if _back_focused else (160, 160, 160), back_rect, width=2, border_radius=10)
    back_lbl = menu_font.render(get_text("back"), True, get_text_color())
    base_surface.blit(back_lbl, (back_rect.centerx - back_lbl.get_width() // 2,
                                 back_rect.centery - back_lbl.get_height() // 2))

    pygame.display.flip()


# -----------------------------
# More Menu + Help (full README scrolling)
# -----------------------------

in_more_menu = False
in_classic_game = False   # True während JumpNDodge Classic (eingebettete Spiel.py-Logik) läuft
in_controls_menu = False
_ctrl_listening = None   # action being recorded
_ctrl_scroll = 0         # vertical scroll offset (px) of the controls list
in_settings_menu = False
in_settings_more_menu = False
in_experimental_menu = False
in_settings_sound_menu = False
in_help = False
in_daily_rewards = False
in_shop_menu = False
in_cosmetic_shop = False
in_loadout_menu = False
in_cosmetic_inventory = False
in_loading = False
in_difficulty_select = False
klassik_mode = False
current_game_mode  = "standard"   # "standard" | "klassik" | "2player"
two_player_submode = "coop"       # "coop" | "vs"
loading_progress = 0
loading_duration = 2000  # 2 Sekunden
loading_start_time = 0
loading_to_daily_rewards = True  # True = zu Daily Rewards, False = zu Hauptmenü
help_scroll = 0
help_lines = []
help_lang = None
last_export_message = ""

# Load README files for help text
def _load_help_texts():
    global HELP_TEXTS
    HELP_TEXTS = {}
    files = {
        'Deutsch': 'README_DE.txt',
        'English': 'README_EN.txt'
    }
    for lang, fname in files.items():
        try:
            with open(fname, 'r', encoding='utf-8') as f:
                HELP_TEXTS[lang] = f.read()
        except Exception:
            HELP_TEXTS[lang] = f'Keine Anleitung für {lang}.'

_load_help_texts()


# =============================
# Tutorial System
# =============================

in_tutorial = False
tutorial_step = 0

# Fallback-Schritte falls tutorial.json manipuliert/fehlt
_TUTORIAL_FALLBACK = {
    "steps": [
        {
            "title": {
                "Deutsch": "Willkommen bei JumpNDodge!",
                "English": "Welcome to JumpNDodge!",
                "Русский": "Добро пожаловать в JumpNDodge!",
                "Türkçe": "JumpNDodge'a Hoş Geldiniz!",
                "Schwiizerdütsch": "Willkomme bi JumpNDodge!"
            },
            "text": {
                "Deutsch": "JumpNDodge ist ein Arcade-Spiel, bei dem du Hindernissen ausweichst und so weit wie möglich kommst. Dieses Tutorial zeigt dir die Grundlagen.",
                "English": "JumpNDodge is an arcade game where you dodge obstacles and get as far as possible. This tutorial shows you the basics.",
                "Русский": "JumpNDodge — аркадная игра, в которой нужно уклоняться от препятствий и пройти как можно дальше. Этот туториал покажет тебе основы.",
                "Türkçe": "JumpNDodge, engellerden kaçınarak mümkün olduğunca ilerlediğiniz bir arcade oyunudur. Bu öğretici size temel bilgileri gösterecek.",
                "Schwiizerdütsch": "JumpNDodge isch es Arcade-Spiel, wo du Hindernis uswychsch und so wiit wie möglich chunnsch. Das Tutorial zeigt dir d Grundlage."
            },
            "hint": None
        },
        {
            "title": {
                "Deutsch": "Steuerung",
                "English": "Controls",
                "Русский": "Управление",
                "Türkçe": "Kontroller",
                "Schwiizerdütsch": "Stüürig"
            },
            "text": {
                "Deutsch": "Benutze diese Tasten, um deinen Charakter zu steuern:",
                "English": "Use these keys to control your character:",
                "Русский": "Используй эти клавиши для управления персонажем:",
                "Türkçe": "Karakterini kontrol etmek için bu tuşları kullan:",
                "Schwiizerdütsch": "Bruuch disi Taste zum din Charakter stüüre:"
            },
            "hint": {
                "Deutsch":        "SPACE = Springen  |  A = Links  |  D = Rechts",
                "English":        "SPACE = Jump  |  A = Left  |  D = Right",
                "Русский":        "SPACE = Прыжок  |  A = Влево  |  D = Вправо",
                "Türkçe":         "SPACE = Zıpla  |  A = Sol  |  D = Sağ",
                "Schwiizerdütsch":"SPACE = Springe  |  A = Links  |  D = Rechts"
            }
        },
        {
            "title": {
                "Deutsch": "Das Hauptmenü",
                "English": "The Main Menu",
                "Русский": "Главное меню",
                "Türkçe": "Ana Menü",
                "Schwiizerdütsch": "S Hauptmenü"
            },
            "text": {
                "Deutsch": "Im Hauptmenü kannst du das Spiel starten, den Shop öffnen, dein Loadout verwalten und die Einstellungen anpassen. Oben links siehst du immer deine Coins.",
                "English": "In the main menu you can start the game, open the shop, manage your loadout, and adjust settings. Your coin count is always shown top-left.",
                "Русский": "В главном меню можно начать игру, открыть магазин, управлять снаряжением и изменить настройки. Количество монет всегда отображается в левом верхнем углу.",
                "Türkçe": "Ana menüde oyunu başlatabilir, mağazayı açabilir, yükünü yönetebilir ve ayarları değiştirebilirsiniz. Coin sayınız sol üstte gösterilir.",
                "Schwiizerdütsch": "Im Hauptmenü chasch s Spiel starte, de Shop öffne, dis Loadout verwalte und d Istellige apasse. Links obe gsiisch immer dini Coins."
            },
            "hint": None
        },
        {
            "title": {
                "Deutsch": "Coins & Shop",
                "English": "Coins & Shop",
                "Русский": "Монеты и магазин",
                "Türkçe": "Coinler ve Mağaza",
                "Schwiizerdütsch": "Coins & Shop"
            },
            "text": {
                "Deutsch": "Sammle Coins während des Spiels und mit den täglichen Belohnungen. Im Shop kannst du Powers und Kosmetik-Items freischalten, die dir im Spiel helfen.",
                "English": "Collect coins during gameplay and with daily rewards. In the shop you can unlock powers and cosmetic items that help you in the game.",
                "Русский": "Собирай монеты во время игры и с ежедневными наградами. В магазине можно разблокировать способности и косметические предметы, которые помогут в игре.",
                "Türkçe": "Oyun sırasında ve günlük ödüllerle coin toplayın. Mağazada oyunda size yardımcı olacak güçler ve kozmetik öğelerin kilidini açabilirsiniz.",
                "Schwiizerdütsch": "Sammel Coins im Spiel und mit de tägleche Belohnige. Im Shop chasch Powers und Kosmetik-Items freischalte, wo dir im Spiel helffe."
            },
            "hint": None
        },
        {
            "title": {
                "Deutsch": "Bereit!",
                "English": "Ready!",
                "Русский": "Готово!",
                "Türkçe": "Hazır!",
                "Schwiizerdütsch": "Bereit!"
            },
            "text": {
                "Deutsch": "Du kennst jetzt die Grundlagen! Viel Spass und versuche, deinen Highscore zu brechen. Das Tutorial findest du jederzeit unter Mehr → Anleitung.",
                "English": "You now know the basics! Have fun and try to beat your highscore. You can find this tutorial anytime under More → Help.",
                "Русский": "Теперь ты знаешь основы! Удачи и постарайся побить свой рекорд. Туториал доступен в разделе Ещё → Помощь.",
                "Türkçe": "Artik temel bilgileri biliyorsunuz! Eglenceli oyunlar dileriz. Bu ogreticiye Daha Fazla → Yardim altinda ulasabilirsiniz.",
                "Schwiizerdütsch": "Du kennsch jetzt d Grundlage! Viel Spass und versüech, din Highscore z breche. Das Tutorial findsch du under Meh → Anleitung."
            },
            "hint": None
        }
    ]
}

def _ensure_tutorial_json():
    """Stellt sicher, dass die Tutorial-Daten (Sektion '_tutorial' in der
    verschlüsselten gameplay.json) vorhanden sind; legt sie sonst neu an."""
    data = load_tutorial_data()
    if data is None or "steps" not in data:
        save_tutorial_data(_TUTORIAL_FALLBACK)

def _get_tutorial_steps():
    _ensure_tutorial_json()
    data = load_tutorial_data(fallback=_TUTORIAL_FALLBACK)
    if not data or "steps" not in data:
        return _TUTORIAL_FALLBACK["steps"]
    return data["steps"]

def _tutorial_lang():
    lang = settings.get("language", "Deutsch")
    return lang if lang in ("Deutsch", "English", "Русский", "Türkçe", "Schwiizerdütsch") else "Deutsch"

def draw_tutorial():
    global in_tutorial, tutorial_step, in_menu
    steps = _get_tutorial_steps()
    lang = _tutorial_lang()
    total = len(steps)
    step = steps[min(tutorial_step, total - 1)]

    # Background
    fill_bg(rings=True)

    # Dimmed overlay panel
    panel_w, panel_h = 560, 320
    panel_x = WIDTH // 2 - panel_w // 2
    panel_y = HEIGHT // 2 - panel_h // 2
    panel_rect = pygame.Rect(panel_x, panel_y, panel_w, panel_h)
    pygame.draw.rect(base_surface, get_panel_color(), panel_rect, border_radius=18)
    border_col = (100, 160, 255) if not is_dark_mode() else (80, 120, 200)
    pygame.draw.rect(base_surface, border_col, panel_rect, width=3, border_radius=18)

    # Step counter
    counter_text = small_font.render(f"{tutorial_step + 1} / {total}", True, (150, 150, 180))
    base_surface.blit(counter_text, (panel_x + panel_w - counter_text.get_width() - 16, panel_y + 14))

    # Title
    title_str = step["title"].get(lang, step["title"].get("Deutsch", ""))
    title_surf = menu_font.render(title_str, True, (80, 140, 255) if not is_dark_mode() else (120, 170, 255))
    base_surface.blit(title_surf, (panel_x + panel_w // 2 - title_surf.get_width() // 2, panel_y + 20))

    # Divider
    pygame.draw.line(base_surface, border_col, (panel_x + 20, panel_y + 60), (panel_x + panel_w - 20, panel_y + 60), 1)

    # Body text (word-wrap)
    body_str = step["text"].get(lang, step["text"].get("Deutsch", ""))
    words = body_str.split()
    lines_out, cur = [], ""
    for w in words:
        test = (cur + " " + w).strip()
        if small_font.size(test)[0] > panel_w - 40:
            lines_out.append(cur)
            cur = w
        else:
            cur = test
    if cur:
        lines_out.append(cur)

    ty = panel_y + 72
    for ln in lines_out:
        surf = small_font.render(ln, True, get_text_color())
        base_surface.blit(surf, (panel_x + 20, ty))
        ty += 24

    # Key hint box — split on | for multi-line, fits in panel
    hint_raw = step.get("hint")
    hint = hint_raw.get(lang, hint_raw.get("Deutsch", "")) if isinstance(hint_raw, dict) else hint_raw
    if hint:
        parts = [p.strip() for p in hint.split("|")] if "|" in hint else [hint]
        part_h, gap_h = 28, 5
        total_hint_h = len(parts) * part_h + (len(parts) - 1) * gap_h
        hint_y0 = panel_y + panel_h - 60 - total_hint_h
        bg_c  = (55, 75, 115) if is_dark_mode() else (215, 228, 255)
        txt_c = (180, 210, 255) if is_dark_mode() else (30, 55, 140)
        for pi, part in enumerate(parts):
            hy = hint_y0 + pi * (part_h + gap_h)
            hbg = pygame.Rect(panel_x + 20, hy, panel_w - 40, part_h)
            pygame.draw.rect(base_surface, bg_c, hbg, border_radius=7)
            hs = small_font.render(part, True, txt_c)
            base_surface.blit(hs, (hbg.centerx - hs.get_width() // 2,
                                   hbg.centery - hs.get_height() // 2))

    # Buttons: Back / Weiter (or Fertig on last step)
    btn_y = panel_y + panel_h - 52
    btn_h = 40

    # "Weiter" / "Fertig"
    is_last = (tutorial_step >= total - 1)
    next_labels = {"Deutsch": "Fertig!" if is_last else "Weiter >>",
                   "English": "Done!" if is_last else "Next >>",
                   "Русский": "Готово!" if is_last else "Далее >>",
                   "Türkçe": "Bitti!" if is_last else "Ileri >>",
                   "Schwiizerdütsch": "Fertig!" if is_last else "Witer >>"}
    next_label = next_labels.get(lang, next_labels["Deutsch"])
    next_col = (80, 200, 120) if is_last else (80, 140, 255)
    next_rect = pygame.Rect(panel_x + panel_w - 160, btn_y, 140, btn_h)
    mouse_pos = get_mouse_pos_logical()
    next_hov = next_rect.collidepoint(mouse_pos)
    draw_ui_button(base_surface, next_rect, next_label, font=menu_font, hovered=next_hov,
                    base_color=next_col, hover_color=tuple(min(255, c + 30) for c in next_col),
                    text_color=(255, 255, 255), border_color=tuple(max(0, c - 40) for c in next_col),
                    hover_border_color=(255, 255, 255))

    # "Zurück" (only if not first step)
    if tutorial_step > 0:
        back_labels = {"Deutsch": "<< Zurück", "English": "<< Back", "Русский": "<< Назад",
                       "Türkçe": "<< Geri", "Schwiizerdütsch": "<< Zrugg"}
        back_label = back_labels.get(lang, "<< Zurück")
        back_rect = pygame.Rect(panel_x + 20, btn_y, 140, btn_h)
        back_hov = back_rect.collidepoint(mouse_pos)
        draw_ui_button(base_surface, back_rect, back_label, font=menu_font, hovered=back_hov,
                        border_color=(160, 160, 180), border_width=1)

    # Progress dots
    dot_y = btn_y + btn_h // 2
    dot_start = WIDTH // 2 - (total * 14) // 2
    for i in range(total):
        col = (80, 140, 255) if i == tutorial_step else ((160, 160, 200) if not is_dark_mode() else (80, 80, 100))
        pygame.draw.circle(base_surface, col, (dot_start + i * 14, dot_y), 4 if i == tutorial_step else 3)

    pygame.display.flip()


def _finish_tutorial():
    global in_tutorial, in_menu, tutorial_step
    stats["tutorial_seen"] = True
    save_section("stats", stats)
    in_tutorial = False
    in_menu = True
    tutorial_step = 0


# =============================
# Language Select Screen (Erster Start)
# =============================

in_language_select = False

_LANG_DATA = [
    {
        "key": "Deutsch",
        "display": "Deutsch",
        "sample": "Hallo! Wie geht's?",
        "flag_colors": [(20, 20, 20), (200, 30, 30), (220, 180, 0)],
        "flag_type": "hstripes",
    },
    {
        "key": "English",
        "display": "English",
        "sample": "Hello! How are you?",
        "flag_colors": [(0, 50, 140), (200, 20, 20), (255, 255, 255)],
        "flag_type": "english",
    },
    {
        "key": "Russisch",
        "display": "Russki",
        "sample": "Privyet! Kak dela?",
        "flag_colors": [(255, 255, 255), (0, 80, 180), (200, 20, 30)],
        "flag_type": "hstripes",
    },
    {
        "key": "Turkce",
        "display": "Turkce",
        "sample": "Merhaba! Nasilsin?",
        "flag_colors": [(200, 20, 30), (255, 255, 255)],
        "flag_type": "turkey",
    },
    {
        "key": "Schwiizerdutsch",
        "display": "Schwiizerdutsch",
        "sample": "Hoi! Wie gohts?",
        "flag_colors": [(190, 20, 30), (255, 255, 255)],
        "flag_type": "swiss",
    },
]
_LANG_KEYS_MAP = {
    "Deutsch": "Deutsch",
    "English": "English",
    "Russisch": "Русский",
    "Turkce": "Türkçe",
    "Schwiizerdutsch": "Schwiizerdütsch",
}

def _draw_flag(surface, rect, flag_type, colors):
    """Zeichnet eine vereinfachte Flagge in ein Rect."""
    x, y, w, h = rect.x, rect.y, rect.width, rect.height
    pygame.draw.rect(surface, (80, 80, 80), rect, border_radius=6)

    if flag_type == "hstripes":
        stripe_h = h // 3
        for i, c in enumerate(colors[:3]):
            r = pygame.Rect(x, y + i * stripe_h, w, stripe_h + (1 if i == 2 else 0))
            pygame.draw.rect(surface, c, r)

    elif flag_type == "english":
        # USA: rote/weisse Streifen + blauer Kanton
        stripe_h = max(1, h // 7)
        for i in range(7):
            c = colors[1] if i % 2 == 0 else colors[2]
            pygame.draw.rect(surface, c, pygame.Rect(x, y + i * stripe_h, w, stripe_h + (h - 7 * stripe_h if i == 6 else 0)))
        canton_w = w * 2 // 5
        canton_h = stripe_h * 4
        pygame.draw.rect(surface, colors[0], pygame.Rect(x, y, canton_w, canton_h))
        for row in range(3):
            for col in range(3 if row % 2 == 0 else 2):
                sx = x + canton_w // 6 + col * (canton_w // 3) + (canton_w // 6 if row % 2 != 0 else 0)
                sy = y + canton_h // 7 + row * (canton_h // 4)
                pygame.draw.circle(surface, colors[2], (int(sx), int(sy)), 2)

    elif flag_type == "turkey":
        import math as _m
        pygame.draw.rect(surface, colors[0], rect)
        cr_cx, cr_cy = x + w * 2 // 5, y + h // 2
        cr_r = h // 4
        # Halbmond
        pygame.draw.circle(surface, colors[1], (cr_cx, cr_cy), cr_r)
        pygame.draw.circle(surface, colors[0], (cr_cx + cr_r // 3, cr_cy), int(cr_r * 0.82))
        # Stern
        st_cx = int(cr_cx + cr_r * 1.2)
        st_cy = cr_cy
        r_out = int(cr_r * 0.45)
        r_in  = int(cr_r * 0.18)
        pts = []
        for i in range(10):
            r = r_out if i % 2 == 0 else r_in
            a = _m.radians(-90 + i * 36)
            pts.append((st_cx + r * _m.cos(a), st_cy + r * _m.sin(a)))
        pygame.draw.polygon(surface, colors[1], pts)

    elif flag_type == "swiss":
        pygame.draw.rect(surface, colors[0], rect)
        # Weisses Kreuz
        cx, cy = x + w // 2, y + h // 2
        arm = max(4, h // 4)
        thick = max(3, h // 8)
        pygame.draw.rect(surface, colors[1], pygame.Rect(cx - thick//2, cy - arm, thick, arm * 2))
        pygame.draw.rect(surface, colors[1], pygame.Rect(cx - arm, cy - thick//2, arm * 2, thick))

    pygame.draw.rect(surface, (40, 40, 40), rect, width=2, border_radius=6)


def draw_language_select():
    fill_bg(rings=True)
    dark = is_dark_mode()
    mouse_pos = get_mouse_pos_logical()
    accent = (80, 140, 255) if not dark else (120, 170, 255)

    _ls_total = 5 + (0 if _lang_from_settings else 2) + 1  # 5 Karten + (2 Hell/Dunkel, bedingt) + Weiter
    _ls_weiter_idx = _ls_total - 1

    # --- Titel (mit Schatten wie Hauptmenü) ---
    t1        = font.render("JumpNDodge", True, accent)
    t1_shadow = font.render("JumpNDodge", True, (80, 80, 110) if dark else (140, 140, 170))
    base_surface.blit(t1_shadow, (WIDTH // 2 - t1.get_width() // 2 + 3, 13))
    base_surface.blit(t1,        (WIDTH // 2 - t1.get_width() // 2,     10))
    t2 = small_font.render(get_text("choose_language"), True, get_text_color())
    base_surface.blit(t2, (WIDTH // 2 - t2.get_width() // 2, 58))

    # --- 2-Zeilen Layout: Reihe 1 = [0,1,2], Reihe 2 = [3,4] zentriert ---
    card_w, card_h = 160, 150
    flag_h = 80
    spacing = 18
    row1 = _LANG_DATA[:3]
    row2 = _LANG_DATA[3:]
    row1_y = 80
    row2_y = row1_y + card_h + 14

    def _row_start_x(n):
        return WIDTH // 2 - (n * card_w + (n - 1) * spacing) // 2

    card_rects = [None] * 5

    def _draw_card(i, ld, cx, cy):
        card_rect = pygame.Rect(cx, cy, card_w, card_h)
        flag_rect = pygame.Rect(cx + 6, cy + 6, card_w - 12, flag_h)
        card_rects[i] = card_rect

        hovered  = card_rect.collidepoint(mouse_pos) or nav_is_focused("language_select", i, _ls_total, default_idx=_ls_weiter_idx)
        selected = (settings.get("language", "Deutsch") == _LANG_KEYS_MAP.get(ld["key"], ld["key"]))

        # Schatten
        pygame.draw.rect(base_surface, (100, 100, 120) if dark else (185, 185, 205),
                         pygame.Rect(cx + 3, cy + 3, card_w, card_h), border_radius=14)
        # Hintergrund
        bg_col = get_button_hover_color() if hovered else get_panel_color()
        if selected:
            bg_col = (38, 78, 42) if dark else (210, 248, 218)
        pygame.draw.rect(base_surface, bg_col, card_rect, border_radius=14)

        # Rahmen
        bw = 3 if (selected or hovered) else 1
        bc = (80, 220, 110) if selected else ((100, 160, 255) if hovered else (160, 160, 180))
        pygame.draw.rect(base_surface, bc, card_rect, width=bw, border_radius=14)

        # Flagge
        _draw_flag(base_surface, flag_rect, ld["flag_type"], ld["flag_colors"])

        # Name (geclippt auf Kartenbreite)
        name_surf = small_font.render(ld["display"], True, get_text_color())
        max_name_w = card_w - 12
        if name_surf.get_width() > max_name_w:
            name_surf = name_surf.subsurface(pygame.Rect(0, 0, max_name_w, name_surf.get_height()))
        base_surface.blit(name_surf, (cx + card_w // 2 - min(name_surf.get_width(), max_name_w) // 2,
                                      cy + flag_h + 10))

        # Beispielsatz (geclippt)
        sample_col = (60, 170, 90) if not dark else (70, 180, 100)
        sample_surf = small_font.render(ld["sample"], True, sample_col)
        max_s_w = card_w - 10
        if sample_surf.get_width() > max_s_w:
            sample_surf = sample_surf.subsurface(pygame.Rect(0, 0, max_s_w, sample_surf.get_height()))
        base_surface.blit(sample_surf, (cx + card_w // 2 - min(sample_surf.get_width(), max_s_w) // 2,
                                        cy + flag_h + 32))

        # Häkchen-Marker
        if selected:
            check_s = small_font.render("✔", True, (80, 220, 110))
            base_surface.blit(check_s, (cx + card_w - check_s.get_width() - 6, cy + 4))

    # Reihe 1
    r1_x = _row_start_x(len(row1))
    for j, ld in enumerate(row1):
        _draw_card(j, ld, r1_x + j * (card_w + spacing), row1_y)

    # Reihe 2
    r2_x = _row_start_x(len(row2))
    for j, ld in enumerate(row2):
        _draw_card(3 + j, ld, r2_x + j * (card_w + spacing), row2_y)

    mode_y = row2_y + card_h + 18
    btn_w, btn_h = 160, 38
    gap = 16
    light_rect = pygame.Rect(-999, -999, 1, 1)
    dark_rect  = pygame.Rect(-999, -999, 1, 1)

    if not _lang_from_settings:
        # --- Hell / Dunkel Auswahl (nur im Tutorial-Flow) ---
        mode_label = small_font.render(get_text("color_scheme") + ":", True, get_text_color())
        base_surface.blit(mode_label, (WIDTH // 2 - mode_label.get_width() // 2, mode_y))

        light_rect = pygame.Rect(WIDTH // 2 - btn_w - gap // 2, mode_y + 28, btn_w, btn_h)
        dark_rect  = pygame.Rect(WIDTH // 2 + gap // 2,          mode_y + 28, btn_w, btn_h)

        light_sel = not dark
        dark_sel  = dark

        for _mi, (rect, label, sel, is_light) in enumerate([(light_rect, get_text("light_mode"), light_sel, True), (dark_rect, get_text("dark_mode"), dark_sel, False)]):
            hov = rect.collidepoint(mouse_pos) or nav_is_focused("language_select", 5 + _mi, _ls_total, default_idx=_ls_weiter_idx)
            if sel:
                bg     = (255, 220, 80) if is_light else (40, 40, 60)
                border = (220, 180, 0)  if is_light else (120, 120, 200)
            else:
                bg     = get_button_hover_color() if hov else get_button_color()
                border = (160, 160, 180)
            pygame.draw.rect(base_surface, bg, rect, border_radius=10)
            pygame.draw.rect(base_surface, border, rect, width=2 if not sel else 3, border_radius=10)
            ls = small_font.render(label, True, (30, 30, 30) if (sel and is_light) else get_text_color())
            base_surface.blit(ls, (rect.centerx - ls.get_width() // 2, rect.centery - ls.get_height() // 2))

        weiter_y = mode_y + 28 + btn_h + 12
    else:
        weiter_y = mode_y + 12

    # --- Weiter-Button ---
    btn_label = get_text("lang_continue")
    weiter_rect = pygame.Rect(WIDTH // 2 - 120, weiter_y, 240, 46)
    w_hov = weiter_rect.collidepoint(mouse_pos) or nav_is_focused("language_select", _ls_weiter_idx, _ls_total, default_idx=_ls_weiter_idx)
    pygame.draw.rect(base_surface, (80, 200, 120) if not w_hov else (100, 230, 140), weiter_rect, border_radius=12)
    ws = menu_font.render(btn_label, True, (255, 255, 255))
    base_surface.blit(ws, (weiter_rect.centerx - ws.get_width() // 2, weiter_rect.centery - ws.get_height() // 2))

    pygame.display.flip()
    return weiter_rect, light_rect, dark_rect, card_rects


def _select_language(lang_key):
    """Setzt die Sprache und speichert sie."""
    real_key = _LANG_KEYS_MAP.get(lang_key, lang_key)
    settings["language"] = real_key
    if real_key in languages:
        global language_index
        language_index = languages.index(real_key)
    save_section("settings", settings)
    update_button_texts()


_lang_from_settings = False

def _finish_language_select():
    global in_language_select, in_tutorial, in_settings_menu, _lang_from_settings
    in_language_select = False
    if _lang_from_settings:
        _lang_from_settings = False
        in_settings_menu = True
    else:
        in_tutorial = True

def open_language_select_from_settings():
    global in_language_select, in_settings_menu, _lang_from_settings
    play_sound(menu_click_sound)
    in_settings_menu = False
    in_language_select = True
    _lang_from_settings = True


# Functions moved to before buttons creation

# Helper to reset menu flags
def set_all_menu_flags(val=False):
    global in_menu, game_running, showing_stats, in_shop, in_loadout, in_admin_login, in_admin_panel, in_more_menu, in_classic_game, in_settings_menu, in_experimental_menu, in_help, in_daily_rewards, in_loading, in_tutorial, in_language_select, in_achievements_screen, in_profile_select, in_profile_name_input, in_profile_hub
    in_menu = val
    game_running = False
    showing_stats = False
    in_shop = False
    in_loadout = False
    in_admin_login = False
    in_admin_panel = False
    in_more_menu = False
    in_classic_game = False
    in_settings_menu = False
    in_experimental_menu = False
    in_help = False
    in_daily_rewards = False
    in_loading = False
    in_tutorial = False
    in_language_select = False
    in_achievements_screen = False
    in_profile_select = False
    in_profile_name_input = False
    in_profile_hub = False

# =============================
# Profile Screens
# =============================

_profile_name_text = ""
_profile_name_msg = ""
_profile_from_settings = False

def _enter_profile(name):
    global in_profile_select, in_menu, in_language_select, in_profile_name_input
    load_profile(name)
    check_local_verified_key_file()
    _flush_pending_online_deletions()
    set_all_menu_flags(False)
    if not stats.get("tutorial_seen", False):
        in_language_select = True
    else:
        in_menu = True
    update_button_texts()

def _open_new_profile_input(from_settings=False):
    global in_profile_name_input, in_profile_select, in_settings_menu, _profile_name_text, _profile_name_msg, _profile_from_settings
    _profile_name_text = ""
    _profile_name_msg = ""
    _profile_from_settings = from_settings
    in_profile_select = False
    in_settings_menu = False
    in_profile_name_input = True

def draw_profile_name_input():
    global _profile_name_text, _profile_name_msg
    fill_bg(rings=True)
    dark = is_dark_mode()
    pw, ph = 520, 280
    px = WIDTH // 2 - pw // 2
    py = HEIGHT // 2 - ph // 2
    panel = pygame.Rect(px, py, pw, ph)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=18)
    pygame.draw.rect(base_surface, (100, 160, 255) if not dark else (80, 120, 200), panel, width=3, border_radius=18)

    title = menu_font.render(get_text("enter_name"), True, (80, 140, 255) if not dark else (120, 170, 255))
    base_surface.blit(title, (px + pw // 2 - title.get_width() // 2, py + 24))

    # Input box
    box = pygame.Rect(px + 40, py + 90, pw - 80, 46)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), box, width=2, border_radius=10)
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "
    inp = menu_font.render(_profile_name_text + cursor, True, get_text_color())
    base_surface.blit(inp, (box.x + 10, box.centery - inp.get_height() // 2))

    # Error msg
    if _profile_name_msg:
        msg = small_font.render(_profile_name_msg, True, (220, 60, 60))
        base_surface.blit(msg, (px + pw // 2 - msg.get_width() // 2, py + 150))

    # Confirm button
    confirm_rect = pygame.Rect(px + pw // 2 - 100, py + ph - 70, 200, 44)
    mp = get_mouse_pos_logical()
    hov = confirm_rect.collidepoint(mp)
    pygame.draw.rect(base_surface, (60, 180, 100) if not hov else (80, 210, 120), confirm_rect, border_radius=12)
    cl = menu_font.render(get_text("confirm"), True, (255, 255, 255))
    base_surface.blit(cl, (confirm_rect.centerx - cl.get_width() // 2, confirm_rect.centery - cl.get_height() // 2))
    pygame.display.flip()
    return confirm_rect

def _confirm_profile_name():
    global _profile_name_text, _profile_name_msg, in_profile_name_input, in_language_select, in_menu, in_profile_select, in_settings_menu, _profile_from_settings
    name = _profile_name_text.strip()
    if not name:
        _profile_name_msg = get_text("name_empty")
        return
    if name in get_all_profiles():
        _profile_name_msg = get_text("name_taken")
        return
    if not is_name_allowed(name):
        _profile_name_msg = get_text("name_filtered")
        return
    create_profile(name)
    in_profile_name_input = False
    in_language_select = True

def draw_profile_select():
    fill_bg(rings=True)
    dark = is_dark_mode()
    all_profiles = get_all_profiles()
    auto_start_name = get_auto_start_profile()
    mp = get_mouse_pos_logical()

    title = font.render(get_text("who_is_playing"), True, get_text_color())
    shadow = font.render(get_text("who_is_playing"), True, (100, 100, 120))
    base_surface.blit(shadow, (WIDTH // 2 - title.get_width() // 2 + 2, 42))
    base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 40))

    items = list(all_profiles.keys()) + ["+"]
    cols = min(3, len(items))
    card_w, card_h = 220, 120
    gap = 24
    total_w = cols * card_w + (cols - 1) * gap
    start_x = WIDTH // 2 - total_w // 2
    start_y = 130

    card_rects = []
    checkbox_rects = []  # (rect, name) - "Immer mit diesem Profil starten" pro Karte
    last_row_bottom = start_y
    for i, name in enumerate(items):
        col = i % cols
        row = i // cols
        cx = start_x + col * (card_w + gap)
        cy = start_y + row * (card_h + gap)
        rect = pygame.Rect(cx, cy, card_w, card_h)
        last_row_bottom = max(last_row_bottom, rect.bottom)
        card_rects.append((rect, name))
        hov = rect.collidepoint(mp) or nav_is_focused("profile_select", i, len(items))

        if name == "+":
            bg = (60, 140, 80) if not hov else (80, 170, 100)
            pygame.draw.rect(base_surface, bg, rect, border_radius=14)
            pygame.draw.rect(base_surface, (100, 220, 140), rect, width=2, border_radius=14)
            plus = font.render("+", True, (255, 255, 255))
            base_surface.blit(plus, (rect.centerx - plus.get_width() // 2, rect.centery - plus.get_height() // 2))
            lbl = small_font.render(get_text("add_profile"), True, (200, 255, 210))
            base_surface.blit(lbl, (rect.centerx - lbl.get_width() // 2, rect.bottom - 26))
        else:
            bg = get_button_hover_color() if hov else get_button_color()
            pygame.draw.rect(base_surface, bg, rect, border_radius=14)
            bord = (120, 180, 255) if not dark else (80, 130, 200)
            pygame.draw.rect(base_surface, bord, rect, width=2, border_radius=14)
            pdata = all_profiles[name]
            pstats = pdata.get("stats", {})
            name_s = menu_font.render(name, True, get_text_color())
            base_surface.blit(name_s, (rect.centerx - name_s.get_width() // 2, rect.y + 16))
            coins_s = small_font.render(f"Coins: {pstats.get('coins', 0)}", True, (180, 180, 100) if not dark else (220, 210, 100))
            rounds_s = small_font.render(f"Runden: {pstats.get('rounds', 0)}", True, get_text_color())
            base_surface.blit(coins_s,  (rect.x + 14, rect.y + 56))
            base_surface.blit(rounds_s, (rect.x + 14, rect.y + 80))

            # Avatar-Vorschau (nur Farbe, keine Kosmetik) - liest die GESPEICHERTEN
            # Settings DIESES Profils direkt aus pdata statt der aktuell aktiven globalen
            # settings/cosmetics_state, da hier auch fremde (nicht geladene) Profile
            # angezeigt werden. Bewusst ohne Kosmetik: draw_player_cosmetics() kennt nur
            # die aktuell aktive globale cosmetics_state, könnte hier also nur die
            # Kosmetik des eigenen/aktiven Profils zeigen - für andere Karten wäre das
            # falsch, deshalb lieber nur der zuverlässig korrekte Farbblock.
            _pcolor_name = pdata.get("settings", {}).get("player_color", "blue")
            _pcolor = PLAYER_COLORS.get(_pcolor_name, BLUE)
            _avatar_rect = pygame.Rect(rect.right - 46, rect.y + 40, 32, 32)
            draw_avatar(base_surface, _avatar_rect, color=_pcolor, cosmetics=False)

            # Delete button (small X top-right of card)
            del_rect = pygame.Rect(rect.right - 28, rect.y + 6, 22, 22)
            del_hov = del_rect.collidepoint(mp)
            pygame.draw.rect(base_surface, (200, 60, 60) if del_hov else (140, 40, 40), del_rect, border_radius=6)
            _m = 5
            pygame.draw.line(base_surface, (255,255,255), (del_rect.x+_m, del_rect.y+_m), (del_rect.right-_m, del_rect.bottom-_m), 2)
            pygame.draw.line(base_surface, (255,255,255), (del_rect.right-_m, del_rect.y+_m), (del_rect.x+_m, del_rect.bottom-_m), 2)

            # "Immer mit diesem Profil starten" - kleine Checkbox unten in der Karte.
            # Exklusiv: nur ein Profil kann gleichzeitig Auto-Start-Ziel sein.
            checked = (auto_start_name == name)
            cb_rect = pygame.Rect(rect.x + 10, rect.bottom - 20, 14, 14)
            checkbox_rects.append((cb_rect, name))
            cb_hov = cb_rect.collidepoint(mp)
            cb_border = (255, 210, 60) if (checked or cb_hov) else (140, 140, 150)
            pygame.draw.rect(base_surface, get_bg_color(), cb_rect, border_radius=3)
            pygame.draw.rect(base_surface, cb_border, cb_rect, width=2, border_radius=3)
            if checked:
                pygame.draw.line(base_surface, (255, 210, 60), (cb_rect.x + 3, cb_rect.centery),
                                  (cb_rect.centerx - 1, cb_rect.bottom - 3), 2)
                pygame.draw.line(base_surface, (255, 210, 60), (cb_rect.centerx - 1, cb_rect.bottom - 3),
                                  (cb_rect.right - 2, cb_rect.y + 2), 2)
            cb_label = tab_font.render(get_text("auto_start_profile"), True, get_text_color())
            base_surface.blit(cb_label, (cb_rect.right + 6, cb_rect.centery - cb_label.get_height() // 2))

    # Hinweis zur Checkbox - OHNE den veralteten "Einstellungen -> Profile"-Verweis (der Pfad
    # existiert nicht mehr, Profil-Wechsel läuft jetzt über den Profil-Hub im Hauptmenü, nicht
    # mehr über die Einstellungen). Die Checkbox ist eh direkt hier auf der Karte bedienbar.
    hint_surf = small_font.render(get_text("auto_start_hint"), True, get_text_color())
    base_surface.blit(hint_surf, (WIDTH // 2 - hint_surf.get_width() // 2, last_row_bottom + 16))

    pygame.display.flip()
    return card_rects, checkbox_rects


# Draw more menu
def draw_more_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    # Titel mit Schatten
    title = font.render(get_text('more'), True, get_text_color())
    title_shadow = font.render(get_text('more'), True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH//2 - title.get_width()//2 + 2, 62))
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 60))

    # Coin-Anzeige
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    
    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))

    # Buttons: Statistics, Admin, Classic, Help, Achievements, Back
    start_x = WIDTH//2 - 180  # Breiter für längere Texte
    start_y = 140
    gap = 58
    if _check_admin_keyfile():
        btn_texts  = [get_text('admin'), get_text('classic_mode'), get_text('help'), get_text('redeem_code_btn'), get_text('back')]
        btn_colors = [(200, 100, 100), (120, 170, 120), (200, 150, 100), (80, 170, 220), (150, 150, 150)]
    else:
        btn_texts  = [get_text('classic_mode'), get_text('help'), get_text('redeem_code_btn'), get_text('back')]
        btn_colors = [(120, 170, 120), (200, 150, 100), (80, 170, 220), (150, 150, 150)]
    
    for i, (text, color) in enumerate(zip(btn_texts, btn_colors)):
        rect = pygame.Rect(start_x, start_y + i*gap, 360, 48)
        is_hovered = rect.collidepoint(mouse_pos) or nav_is_focused("more_menu", i, len(btn_texts))
        draw_ui_button(base_surface, rect, text, font=menu_font, hovered=is_hovered,
                        border_color=color, hover_border_color=color, hover_border_width=3)


    # Footer (theme-aware - vorher immer hell, egal ob Dark Mode aktiv war)
    footer_bg = pygame.Rect(0, HEIGHT - 30, WIDTH, 30)
    pygame.draw.rect(base_surface, get_panel_color(), footer_bg)
    pygame.draw.line(base_surface, (90, 90, 100) if is_dark_mode() else (200, 200, 200), (0, HEIGHT - 30), (WIDTH, HEIGHT - 30), 1)

    footer_text = small_font.render(get_text("back_to_main"), True, get_text_color())
    base_surface.blit(footer_text, (WIDTH // 2 - footer_text.get_width() // 2, HEIGHT - 25))
    pygame.display.flip()


def draw_profile_hub():
    """Profil-Hub: Avatar-Vorschau + Farbwahl (5 Töne, keine Trails - siehe draw_avatar)
    und Sprungbretter zu Kosmetik-Inventar/Statistik/Profil-Wechsel (bestehende Screens,
    unverändert). Ersetzt die früheren getrennten Zugänge im Mehr-Menü (Stats) und in
    den Einstellungen (Profile-Button)."""
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    dark = is_dark_mode()

    title = font.render(get_text('profile_hub_title'), True, get_text_color())
    title_shadow = font.render(get_text('profile_hub_title'), True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH // 2 - title.get_width() // 2 + 2, 22))
    base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 20))

    # Coin-Anzeige
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins', 0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))

    # Avatar-Vorschau (Block in Wunschfarbe + ausgerüstete Kopfbedeckung/Augen/Nase/Mund).
    # Größe bewusst identisch zum echten Gameplay-Spieler (50x50, siehe run_game()) -
    # draw_player_cosmetics() positioniert Hüte/Augen/etc. mit absoluten Pixel-Offsets,
    # die auf diese Größe abgestimmt sind, damit die Vorschau exakt wie im Spiel aussieht.
    avatar_size = 50
    avatar_rect = pygame.Rect(WIDTH // 2 - avatar_size // 2, 100, avatar_size, avatar_size)
    avatar_shadow = pygame.Rect(avatar_rect.x + 4, avatar_rect.y + 4, avatar_size, avatar_size)
    pygame.draw.rect(base_surface, (100, 100, 150), avatar_shadow, border_radius=10)
    draw_avatar(base_surface, avatar_rect)

    # Profilname
    name_surf = menu_font.render(current_profile_name or "-", True, get_text_color())
    base_surface.blit(name_surf, (WIDTH // 2 - name_surf.get_width() // 2, avatar_rect.bottom + 8))

    # Farb-Swatches
    swatch_names = ["blue", "darkgreen", "lightblue", "pink", "orange", "yellow"]
    swatch_size = 40
    swatch_gap = 14
    total_w = len(swatch_names) * swatch_size + (len(swatch_names) - 1) * swatch_gap
    sx0 = WIDTH // 2 - total_w // 2
    sy = avatar_rect.bottom + 44
    swatch_rects = []
    current_color_name = settings.get("player_color", "blue")
    for i, cname in enumerate(swatch_names):
        srect = pygame.Rect(sx0 + i * (swatch_size + swatch_gap), sy, swatch_size, swatch_size)
        swatch_rects.append(srect)
        pygame.draw.rect(base_surface, PLAYER_COLORS[cname], srect, border_radius=8)
        if cname == current_color_name:
            pygame.draw.rect(base_surface, (255, 255, 255) if dark else (0, 0, 0), srect, width=3, border_radius=8)
        elif srect.collidepoint(mouse_pos):
            pygame.draw.rect(base_surface, (200, 200, 200), srect, width=2, border_radius=8)

    # Buttons: Kosmetik ändern, Statistik, Profil wechseln, Zurück
    start_x = WIDTH // 2 - 180
    start_y = sy + swatch_size + 24
    gap = 48  # bei 5 Buttons war hier noch 56/+30 - beides reduziert, seit
              # Freunde/Clan dazukamen. Buttons sind 44px hoch, gap ist der
              # Start-zu-Start-Abstand - unter 44 überlappen sich die Buttons
              # (das war ein echter Bug: 42 < 44). 48 lässt 4px sichtbaren
              # Abstand und passt bei 7 Buttons noch knapp ins 800x600-Fenster.
    btn_texts = [get_text('stats'), get_text('switch_profile'), get_text('online_leaderboard_btn'),
                 get_text('friends_hub_title'), get_text('clan_hub_title'),
                 get_text('rename_btn'), get_text('back')]
    btn_colors = [(100, 150, 200), (200, 150, 100), (100, 180, 160), (100, 190, 130), (210, 175, 80),
                  (170, 130, 200), (150, 150, 150)]
    btn_rects = []
    _hub_nav_total = len(swatch_names) + len(btn_texts)
    for i, (text, color) in enumerate(zip(btn_texts, btn_colors)):
        rect = pygame.Rect(start_x, start_y + i * gap, 360, 44)
        btn_rects.append(rect)
        is_hovered = rect.collidepoint(mouse_pos) or nav_is_focused("profile_hub", len(swatch_names) + i, _hub_nav_total)
        draw_ui_button(base_surface, rect, text, font=menu_font, hovered=is_hovered,
                        border_color=color, hover_border_color=color, hover_border_width=3)

    pygame.display.flip()
    return swatch_rects, swatch_names, btn_rects


# =============================
# Online-Leaderboard-Screens
# =============================

def _draw_wrapped_text(text, x, y, max_w, font_obj, color, line_gap=4):
    """Rendert Text mit einfachem Wortumbruch innerhalb von max_w Pixeln, startend
    bei (x, y). Gibt die y-Position nach der letzten Zeile zurück."""
    words = text.split()
    lines = []
    line = ""
    for w in words:
        test = (line + " " + w).strip()
        if font_obj.size(test)[0] <= max_w:
            line = test
        else:
            if line:
                lines.append(line)
            line = w
    if line:
        lines.append(line)
    for ln in lines:
        line_s = font_obj.render(ln, True, color)
        base_surface.blit(line_s, (x, y))
        y += line_s.get_height() + line_gap
    return y


def _draw_verified_badge(surface, cx, cy, radius=9):
    """Zeichnet ein blaues Verified-Häkchen zentriert bei (cx, cy)."""
    pygame.draw.circle(surface, (60, 150, 255), (cx, cy), radius)
    p1 = (cx - radius * 0.5, cy + radius * 0.05)
    p2 = (cx - radius * 0.1, cy + radius * 0.45)
    p3 = (cx + radius * 0.55, cy - radius * 0.35)
    pygame.draw.lines(surface, (255, 255, 255), False, [p1, p2, p3], max(2, radius // 4))


def _lb_start_fetch():
    """Holt die Rangliste für die aktuell gewählte Schwierigkeit (nur Standard-
    Modus, siehe Kommentar bei _lb_difficulty_idx). _lb_request_id verhindert,
    dass eine veraltete Antwort eine inzwischen neu gewählte Schwierigkeit
    überschreibt."""
    global _lb_loading, _lb_request_id, _lb_entries, _lb_my_rank
    _lb_request_id += 1
    _lb_my_rank = None
    my_id = _lb_request_id
    mode = "standard"
    difficulty = difficulties[_lb_difficulty_idx]

    if _lb_friends_only and not friends_local:
        # Nichts zu holen - direkt Leerzustand zeigen statt unnötigem Request.
        _lb_loading = False
        _lb_entries = []
        return
    _lb_loading = True

    def _on_done(result):
        global _lb_loading, _lb_entries
        if my_id != _lb_request_id:
            return
        _lb_loading = False
        _lb_entries = result.get("entries", []) if (result and result.get("ok")) else []

    def _on_rank_done(result):
        global _lb_my_rank
        if my_id != _lb_request_id:
            return
        if result and result.get("ok"):
            _lb_my_rank = (result.get("rank"), result.get("score"))
        else:
            _lb_my_rank = (None, None)

    names = None
    if _lb_friends_only:
        names = list(friends_local)
        if current_profile_name and current_profile_name not in names:
            names.append(current_profile_name)
    fetch_leaderboard(mode, difficulty, names=names, on_done=_on_done)
    fetch_my_rank(mode, difficulty, on_done=_on_rank_done)


_friends_local_pending = None  # Neue Freundesliste vom Server, von
# _refresh_friends_local()s on_done (Hintergrund-Thread) gesetzt - im Main-
# Thread über _apply_friends_local_pending() angewendet (gleiches Race-
# Condition-Muster wie _rename_pending_local_name, save_current_profile()
# darf nicht aus einem Hintergrund-Thread laufen).


def _refresh_friends_local():
    """Holt die bestätigte Freundesliste vom Server und ersetzt friends_local
    komplett damit - friends_local ist jetzt eine Cache-Kopie des Servers
    (echte Freundschaften brauchen eine Anfrage/Bestätigung, siehe
    send_friend_request_online()), keine rein lokale Bookmark-Liste mehr."""
    if not current_profile_name:
        return

    def _on_done(result):
        global _friends_local_pending
        if result and result.get("ok"):
            _friends_local_pending = list(result.get("friends", []))

    fetch_friends_list(current_profile_name, on_done=_on_done)


def _apply_friends_local_pending():
    global friends_local, _friends_local_pending
    if _friends_local_pending is not None:
        friends_local = _friends_local_pending
        _friends_local_pending = None
        save_current_profile()


def draw_online_leaderboard():
    global _lb_row_rects
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    _lb_row_rects = []

    title_txt = get_text("online_leaderboard_btn")
    title_s = font.render(title_txt, True, get_text_color())
    shadow_s = font.render(title_txt, True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    search_rect = pygame.Rect(20, 15, 110, 35)
    draw_ui_button(base_surface, search_rect, get_text("search"), font=small_font,
                    hovered=search_rect.collidepoint(mouse_pos), radius=10)

    # Alle/Freunde-Umschalter - filtert die Rangliste auf friends_local (siehe
    # _lb_start_fetch()). Kein eigener Tab noetig, wenn (noch) keine Freunde
    # markiert sind - Klick zeigt dann einfach den Leerzustand.
    all_tab_rect = pygame.Rect(60, 65, 100, 36)
    friends_tab_rect = pygame.Rect(166, 65, 100, 36)
    for rect, active, label in [
        (all_tab_rect, not _lb_friends_only, get_text("all_tab_lb")),
        (friends_tab_rect, _lb_friends_only, get_text("friends_tab")),
    ]:
        hov = rect.collidepoint(mouse_pos)
        if active:
            pygame.draw.rect(base_surface, get_button_color(), rect, border_radius=10)
            pygame.draw.rect(base_surface, (100, 160, 255), rect, width=2, border_radius=10)
        else:
            pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), rect, border_radius=10)
            pygame.draw.rect(base_surface, (180, 180, 180), rect, width=1, border_radius=10)
        tab_s = small_font.render(label, True, get_text_color())
        base_surface.blit(tab_s, (rect.centerx - tab_s.get_width() // 2, rect.centery - tab_s.get_height() // 2))

    # Schwierigkeits-Umschalter - rein zum Durchblättern der Rangliste, unabhängig
    # von der echten Spieleinstellung difficulty_index. Nur Standard-Modus, siehe
    # Kommentar bei _lb_difficulty_idx.
    diff_label = get_difficulty_display_name(difficulties[_lb_difficulty_idx])
    diff_rect = pygame.Rect(WIDTH // 2 - 95, 65, 190, 36)
    diff_left = pygame.Rect(diff_rect.x - 32, diff_rect.y, 26, 36)
    diff_right = pygame.Rect(diff_rect.right + 6, diff_rect.y, 26, 36)

    pygame.draw.rect(base_surface, get_panel_color(), diff_rect, border_radius=10)
    pygame.draw.rect(base_surface, (180, 180, 180), diff_rect, width=2, border_radius=10)
    lbl_s = small_font.render(diff_label, True, get_text_color())
    base_surface.blit(lbl_s, (diff_rect.centerx - lbl_s.get_width() // 2, diff_rect.centery - lbl_s.get_height() // 2))
    for arrow_rect, arrow_txt in [(diff_left, "<"), (diff_right, ">")]:
        draw_ui_button(base_surface, arrow_rect, arrow_txt, font=small_font,
                        hovered=arrow_rect.collidepoint(mouse_pos), radius=6, border_width=1)

    header_y = 112
    base_surface.blit(small_font.render("#", True, (140, 140, 160)), (72, header_y))
    base_surface.blit(small_font.render("Name", True, (140, 140, 160)), (120, header_y))
    base_surface.blit(small_font.render(get_text("table_highscore"), True, (140, 140, 160)), (330, header_y))
    base_surface.blit(small_font.render(get_text("powers_used_label"), True, (140, 140, 160)), (420, header_y))

    content_top = 136
    fade_bottom = HEIGHT - 90  # Platz fürs Rang-Dock am unteren Rand lassen

    if _lb_loading or _lb_entries is None:
        loading_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, content_top + 30))
    elif not _lb_entries:
        empty_key = "no_friends_yet" if (_lb_friends_only and not friends_local) else "no_entries_yet"
        empty_s = small_font.render(get_text(empty_key), True, get_text_color())
        base_surface.blit(empty_s, (WIDTH // 2 - empty_s.get_width() // 2, content_top + 30))
    else:
        y = content_top
        row_h = 38
        for entry in _lb_entries:
            if y + row_h > fade_bottom:
                break
            row_rect = pygame.Rect(60, y, WIDTH - 120, row_h - 6)
            _lb_row_rects.append((pygame.Rect(row_rect), entry.get("name", "")))
            hov = row_rect.collidepoint(mouse_pos)
            pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), row_rect, border_radius=8)
            pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)

            rank = entry.get("rank", 0)
            medal_colors = {1: (255, 210, 60), 2: (200, 200, 210), 3: (205, 140, 80)}
            if rank in medal_colors:
                pygame.draw.circle(base_surface, medal_colors[rank], (row_rect.x + 20, row_rect.centery), 12)
                pygame.draw.circle(base_surface, (0, 0, 0, 60), (row_rect.x + 20, row_rect.centery), 12, width=1)
                rank_s = small_font.render(str(rank), True, (40, 30, 10))
                base_surface.blit(rank_s, (row_rect.x + 20 - rank_s.get_width() // 2, row_rect.centery - rank_s.get_height() // 2))
            else:
                rank_s = small_font.render(f"#{rank}", True, get_text_color())
                base_surface.blit(rank_s, (row_rect.x + 12, row_rect.centery - rank_s.get_height() // 2))

            name_s = small_font.render(entry.get("name", ""), True, get_text_color())
            base_surface.blit(name_s, (row_rect.x + 60, row_rect.centery - name_s.get_height() // 2))
            tag_x = row_rect.x + 68 + name_s.get_width()
            if entry.get("verified"):
                _draw_verified_badge(base_surface, tag_x, row_rect.centery, radius=8)
                tag_x += 16
            if entry.get("clan_tag"):
                clan_tag_s = small_font.render(f"[{entry['clan_tag']}]", True, (120, 160, 220))
                base_surface.blit(clan_tag_s, (tag_x + 6, row_rect.centery - clan_tag_s.get_height() // 2))

            score_s = small_font.render(str(entry.get("score", 0)), True, get_text_color())
            base_surface.blit(score_s, (row_rect.x + 270, row_rect.centery - score_s.get_height() // 2))

            powers_txt = ", ".join(entry.get("powers_used") or []) or "-"
            max_w = row_rect.right - (row_rect.x + 360) - 10
            if small_font.size(powers_txt)[0] > max_w:
                while powers_txt and small_font.size(powers_txt + "…")[0] > max_w:
                    powers_txt = powers_txt[:-1]
                powers_txt += "…"
            powers_s = small_font.render(powers_txt, True, (150, 150, 170))
            base_surface.blit(powers_s, (row_rect.x + 360, row_rect.centery - powers_s.get_height() // 2))

            y += row_h

    # Rang-Dock: eigener Rang in diesem Bucket, angepinnt am unteren Rand -
    # immer sichtbar, auch wenn man ausserhalb der oben angezeigten Top-50 ist.
    dock_rect = pygame.Rect(60, HEIGHT - 72, WIDTH - 120, 52)
    dock_hov = current_profile_name is not None and dock_rect.collidepoint(mouse_pos)
    pygame.draw.rect(base_surface, get_button_hover_color() if dock_hov else get_panel_color(), dock_rect, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), dock_rect, width=2, border_radius=10)

    if _lb_my_rank is None:
        dock_txt = get_text("loading")
    else:
        my_rank, my_score = _lb_my_rank
        if my_rank is None:
            dock_txt = f"{current_profile_name or '-'}: {get_text('no_entries_yet')}"
        else:
            dock_txt = f"#{my_rank}  {current_profile_name or '-'}: {my_score:,}".replace(",", ".")
    dock_s = small_font.render(dock_txt, True, get_text_color())
    base_surface.blit(dock_s, (dock_rect.x + 16, dock_rect.centery - dock_s.get_height() // 2))

    pygame.display.flip()
    return back_rect, search_rect, diff_left, diff_right, dock_rect, all_tab_rect, friends_tab_rect


def online_leaderboard_loop():
    global in_online_leaderboard, _lb_difficulty_idx, _lb_friends_only
    clock = pygame.time.Clock()
    _lb_start_fetch()
    _refresh_friends_local()
    _last_key = (_lb_difficulty_idx, _lb_friends_only)
    while in_online_leaderboard:
        back_rect, search_rect, diff_left, diff_right, dock_rect, all_tab_rect, friends_tab_rect = draw_online_leaderboard()
        _apply_friends_local_pending()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_to_hub_from_online_leaderboard()
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    back_to_hub_from_online_leaderboard()
                elif search_rect.collidepoint(pos):
                    open_player_search_from_leaderboard()
                elif all_tab_rect.collidepoint(pos):
                    _lb_friends_only = False
                    play_sound(click_sound)
                elif friends_tab_rect.collidepoint(pos):
                    _lb_friends_only = True
                    play_sound(click_sound)
                elif diff_left.collidepoint(pos):
                    _lb_difficulty_idx = (_lb_difficulty_idx - 1) % len(difficulties)
                    play_sound(click_sound)
                elif diff_right.collidepoint(pos):
                    _lb_difficulty_idx = (_lb_difficulty_idx + 1) % len(difficulties)
                    play_sound(click_sound)
                elif dock_rect.collidepoint(pos) and current_profile_name:
                    open_public_profile(current_profile_name, "in_online_leaderboard")
                else:
                    for rect, name in _lb_row_rects:
                        if rect.collidepoint(pos):
                            open_public_profile(name, "in_online_leaderboard")
                            break
        new_key = (_lb_difficulty_idx, _lb_friends_only)
        if new_key != _last_key and in_online_leaderboard:
            _last_key = new_key
            _lb_start_fetch()
        clock.tick(60)


def _search_trigger(query):
    global _search_loading, _search_request_id, _search_results
    query = query.strip()
    if not query:
        _search_results = []
        _search_loading = False
        return
    _search_loading = True
    _search_request_id += 1
    my_id = _search_request_id

    def _on_done(result):
        global _search_loading, _search_results
        if my_id != _search_request_id:
            return
        _search_loading = False
        _search_results = result.get("results", []) if (result and result.get("ok")) else []

    search_players_online(query, on_done=_on_done)


def draw_player_search():
    global _search_row_rects
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    _search_row_rects = []

    title_s = font.render(get_text("player_search_title"), True, get_text_color())
    shadow_s = font.render(get_text("player_search_title"), True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    box = pygame.Rect(WIDTH // 2 - 220, 75, 440, 46)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), box, width=2, border_radius=10)
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "
    inp = menu_font.render(_search_query + cursor, True, get_text_color())
    base_surface.blit(inp, (box.x + 12, box.centery - inp.get_height() // 2))

    content_top = 140
    fade_bottom = HEIGHT - 45

    if _search_loading:
        loading_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, content_top + 20))
    elif _search_results is not None and not _search_results:
        empty_s = small_font.render(get_text("player_not_found"), True, get_text_color())
        base_surface.blit(empty_s, (WIDTH // 2 - empty_s.get_width() // 2, content_top + 20))
    elif _search_results:
        y = content_top
        row_h = 44
        for entry in _search_results:
            if y + row_h > fade_bottom:
                break
            name = entry.get("name", "")
            row_rect = pygame.Rect(WIDTH // 2 - 220, y, 440, row_h - 6)
            _search_row_rects.append((pygame.Rect(row_rect), name))
            hov = row_rect.collidepoint(mouse_pos)
            pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), row_rect, border_radius=8)
            pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)
            name_s = menu_font.render(name, True, get_text_color())
            base_surface.blit(name_s, (row_rect.x + 14, row_rect.centery - name_s.get_height() // 2))
            tag_x = row_rect.x + 20 + name_s.get_width()
            if entry.get("verified"):
                _draw_verified_badge(base_surface, tag_x, row_rect.centery, radius=9)
                tag_x += 18
            if entry.get("clan_tag"):
                clan_tag_s = small_font.render(f"[{entry['clan_tag']}]", True, (120, 160, 220))
                base_surface.blit(clan_tag_s, (tag_x + 6, row_rect.centery - clan_tag_s.get_height() // 2))
            y += row_h

    pygame.display.flip()
    return back_rect


def player_search_loop():
    global in_player_search, _search_query
    clock = pygame.time.Clock()
    while in_player_search:
        back_rect = draw_player_search()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_BACKSPACE:
                    _search_query = _search_query[:-1]
                    _search_trigger(_search_query)
                elif is_back_key(event.key, allow_backspace=False):
                    back_from_player_search()
                elif event.key != pygame.K_RETURN:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(_search_query) < 20:
                        _search_query += ch
                        _search_trigger(_search_query)
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    back_from_player_search()
                else:
                    for rect, name in _search_row_rects:
                        if rect.collidepoint(pos):
                            open_public_profile(name, "in_player_search")
                            break
        clock.tick(60)


def _public_profile_start_fetch():
    global _public_profile_loading, _public_profile_data
    _public_profile_loading = True
    _public_profile_data = None
    name = _public_profile_name

    def _on_done(result):
        global _public_profile_loading, _public_profile_data
        if name != _public_profile_name:
            return  # zwischenzeitlich wurde ein anderes Profil geöffnet
        _public_profile_loading = False
        _public_profile_data = result if (result and result.get("ok")) else False

    fetch_public_profile(name, on_done=_on_done)


def _format_playtime(seconds):
    seconds = int(seconds or 0)
    hours, rem = divmod(seconds, 3600)
    minutes = rem // 60
    return f"{hours}h {minutes}m" if hours > 0 else f"{minutes}m"


def _draw_section_card(surface, rect, title, accent_color, dark):
    """Karten-Panel mit farbigem Akzentbalken oben + Titel. Gibt die y-Position
    zurück, ab der der Karteninhalt beginnen sollte."""
    pygame.draw.rect(surface, get_panel_color(), rect, border_radius=14)
    pygame.draw.rect(surface, (75, 75, 100) if dark else (215, 215, 230), rect, width=1, border_radius=14)
    accent_rect = pygame.Rect(rect.x, rect.y, rect.w, 5)
    pygame.draw.rect(surface, accent_color, accent_rect, border_top_left_radius=14, border_top_right_radius=14)
    title_s = small_font.render(title, True, accent_color)
    surface.blit(title_s, (rect.x + 16, rect.y + 15))
    return rect.y + 15 + title_s.get_height() + 8


def draw_public_profile():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    dark = is_dark_mode()

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    hov_back = back_rect.collidepoint(mouse_pos)
    pygame.draw.rect(base_surface, get_button_hover_color() if hov_back else get_button_color(), back_rect, border_radius=10)
    pygame.draw.rect(base_surface, (180, 180, 180), back_rect, width=2, border_radius=10)
    back_lbl = small_font.render(get_text("back"), True, get_text_color())
    base_surface.blit(back_lbl, (back_rect.centerx - back_lbl.get_width() // 2, back_rect.centery - back_lbl.get_height() // 2))

    if _public_profile_loading or _public_profile_data is None:
        loading_s = font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, HEIGHT // 2 - loading_s.get_height() // 2))
        pygame.display.flip()
        return back_rect, None, None, None

    if _public_profile_data is False:
        msg_s = font.render(get_text("player_not_found"), True, get_text_color())
        base_surface.blit(msg_s, (WIDTH // 2 - msg_s.get_width() // 2, HEIGHT // 2 - msg_s.get_height() // 2))
        pygame.display.flip()
        return back_rect, None, None, None

    player = _public_profile_data.get("player", {})

    # Freund hinzufügen/entfernen - nicht beim eigenen Profil
    friend_rect = None
    message_rect = None
    viewed_name = player.get("name")
    if viewed_name and viewed_name != current_profile_name:
        is_friend = viewed_name in friends_local
        friend_rect = pygame.Rect(20, 15, 130, 35)
        hov_friend = friend_rect.collidepoint(mouse_pos)
        base_col = (200, 100, 100) if is_friend else (100, 180, 120)
        hov_col = (220, 120, 120) if is_friend else (120, 200, 140)
        pygame.draw.rect(base_surface, hov_col if hov_friend else base_col, friend_rect, border_radius=10)
        pygame.draw.rect(base_surface, (180, 180, 180), friend_rect, width=2, border_radius=10)
        friend_lbl = small_font.render(get_text("remove_friend_btn") if is_friend else get_text("add_friend_btn"), True, (255, 255, 255))
        base_surface.blit(friend_lbl, (friend_rect.centerx - friend_lbl.get_width() // 2, friend_rect.centery - friend_lbl.get_height() // 2))

        if is_friend:
            message_rect = pygame.Rect(160, 15, 130, 35)
            hov_msg = message_rect.collidepoint(mouse_pos)
            pygame.draw.rect(base_surface, (140, 180, 240) if hov_msg else (120, 160, 220), message_rect, border_radius=10)
            pygame.draw.rect(base_surface, (180, 180, 180), message_rect, width=2, border_radius=10)
            msg_lbl = small_font.render(get_text("message_btn"), True, (255, 255, 255))
            base_surface.blit(msg_lbl, (message_rect.centerx - msg_lbl.get_width() // 2, message_rect.centery - msg_lbl.get_height() // 2))
        elif _public_profile_friend_msg:
            fmsg_s = small_font.render(_public_profile_friend_msg, True, (210, 190, 90))
            base_surface.blit(fmsg_s, (20, 54))
    avatar_color = PLAYER_COLORS.get(player.get("player_color") or "blue", PLAYER_COLORS.get("blue"))

    # Avatar mit weichem Farb-Glow dahinter (mehrere Kreise mit abnehmender Deckkraft)
    avatar_size = 56
    avatar_cx, avatar_cy = WIDTH // 2, 78
    glow = pygame.Surface((220, 220), pygame.SRCALPHA)
    for r, a in [(100, 22), (75, 30), (52, 40)]:
        pygame.draw.circle(glow, (*avatar_color, a), (110, 110), r)
    base_surface.blit(glow, (avatar_cx - 110, avatar_cy - 110))
    avatar_rect = pygame.Rect(avatar_cx - avatar_size // 2, avatar_cy - avatar_size // 2, avatar_size, avatar_size)
    avatar_shadow = pygame.Rect(avatar_rect.x + 4, avatar_rect.y + 4, avatar_size, avatar_size)
    pygame.draw.rect(base_surface, (100, 100, 150), avatar_shadow, border_radius=12)
    draw_avatar(base_surface, avatar_rect, color=avatar_color, cosmetics=True, cosmetics_overrides=player.get("cosmetics") or {})

    title_txt = player.get("name") or "-"
    title_s = font.render(title_txt, True, get_text_color())
    shadow_s = font.render(title_txt, True, (100, 100, 120))
    title_y = avatar_rect.bottom + 12
    title_x = WIDTH // 2 - title_s.get_width() // 2
    base_surface.blit(shadow_s, (title_x + 2, title_y + 2))
    base_surface.blit(title_s, (title_x, title_y))
    if player.get("verified"):
        _draw_verified_badge(base_surface, title_x + title_s.get_width() + 18, title_y + title_s.get_height() // 2, radius=12)

    clan_tag_rect = None
    clan_tag = player.get("clan_tag")
    if clan_tag:
        tag_s = small_font.render(f"[{clan_tag}]", True, (120, 160, 220))
        clan_tag_rect = pygame.Rect(0, 0, tag_s.get_width() + 20, tag_s.get_height() + 10)
        clan_tag_rect.centerx = WIDTH // 2
        clan_tag_rect.y = title_y + title_s.get_height() + 6
        hov_tag = clan_tag_rect.collidepoint(mouse_pos)
        pygame.draw.rect(base_surface, get_button_hover_color() if hov_tag else get_panel_color(), clan_tag_rect, border_radius=8)
        pygame.draw.rect(base_surface, (120, 160, 220), clan_tag_rect, width=1, border_radius=8)
        base_surface.blit(tag_s, (clan_tag_rect.centerx - tag_s.get_width() // 2, clan_tag_rect.centery - tag_s.get_height() // 2))

    # 2x2 Karten-Grid: Stats / Powers oben, Highscores / Cosmetics unten
    grid_top = (clan_tag_rect.bottom + 14) if clan_tag_rect else (title_y + title_s.get_height() + 22)
    gap = 20
    col_w = (WIDTH - 2 * 40 - gap) // 2
    row_h = (HEIGHT - grid_top - 30 - gap) // 2
    left_x = 40
    right_x = left_x + col_w + gap
    row2_y = grid_top + row_h + gap

    stats_rect = pygame.Rect(left_x, grid_top, col_w, row_h)
    powers_rect = pygame.Rect(right_x, grid_top, col_w, row_h)
    scores_rect = pygame.Rect(left_x, row2_y, col_w, row_h)
    cosmetics_rect = pygame.Rect(right_x, row2_y, col_w, row_h)

    p_stats = player.get("stats") or {}
    c_top = _draw_section_card(base_surface, stats_rect, get_text("stats"), (100, 170, 255), dark)
    stat_rows = [
        (get_text("coins"), f"{p_stats.get('coins', 0):,}".replace(",", ".")),
        (get_text("rounds_played"), p_stats.get("total_rounds", 0)),
        (get_text("1ups_collected"), p_stats.get("total_1ups", 0)),
    ]
    y = c_top
    for label, value in stat_rows:
        row_s = small_font.render(f"{label}: {value}", True, get_text_color())
        base_surface.blit(row_s, (stats_rect.x + 16, y))
        y += 24
    playtime_s = small_font.render(f"{get_text('play_time_label')}: {_format_playtime(p_stats.get('total_time_played', 0))}", True, get_text_color())
    base_surface.blit(playtime_s, (stats_rect.x + 16, y))

    c_top = _draw_section_card(base_surface, powers_rect, get_text("owned_powers_label"), (255, 190, 80), dark)
    owned_powers = player.get("owned_powers") or []
    _draw_wrapped_text(", ".join(owned_powers) if owned_powers else "-", powers_rect.x + 16, c_top, col_w - 32, small_font, get_text_color())

    c_top = _draw_section_card(base_surface, scores_rect, get_text("stat_highscores"), (140, 220, 140), dark)
    scores_by_diff = {
        s.get("difficulty"): s.get("score", 0)
        for s in (player.get("scores") or []) if s.get("mode") == "standard"
    }
    y = c_top
    for diff in difficulties:
        if y + 19 > scores_rect.bottom - 8:
            break
        score_val = scores_by_diff.get(diff)
        score_txt = f"{score_val:,}".replace(",", ".") if score_val is not None else "-"
        row_txt = f"{get_difficulty_display_name(diff)}: {score_txt}"
        row_s = small_font.render(row_txt, True, get_text_color())
        base_surface.blit(row_s, (scores_rect.x + 16, y))
        y += 19

    c_top = _draw_section_card(base_surface, cosmetics_rect, get_text("owned_cosmetics_label"), (220, 140, 220), dark)
    owned_cosmetics = (player.get("cosmetics") or {}).get("owned") or []
    cosmetic_names = [get_cosmetic_name(c) for c in owned_cosmetics]
    _draw_wrapped_text(", ".join(cosmetic_names) if cosmetic_names else "-", cosmetics_rect.x + 16, c_top, col_w - 32, small_font, get_text_color())

    pygame.display.flip()
    return back_rect, friend_rect, message_rect, clan_tag_rect


_friend_action_pending = None  # Name von send_friend_request_online()s on_done
# (Hintergrund-Thread) gesetzt, im Main-Thread (public_profile_loop()) via
# _add_friend_local() angewendet - gleiches Race-Condition-Muster wie
# _rename_pending_local_name. Wird nur gesetzt, wenn der Server sofort
# "accepted" zurückgibt (symmetrisches Auto-Accept, siehe server/app.py
# friend_request()) - bei "pending" bleibt friends_local bewusst unverändert,
# bis die Gegenseite die Anfrage annimmt, sonst würde eine unbestätigte
# Anfrage fälschlich als Freundschaft in den Freunde-Filter der Rangliste
# einfliessen.
_public_profile_friend_msg = ""


def _add_friend_local(name):
    global friends_local
    if name not in friends_local:
        friends_local.append(name)
        save_current_profile()


def _remove_friend_local(name):
    global friends_local
    if name in friends_local:
        friends_local.remove(name)
        save_current_profile()


def _send_friend_request(name):
    global _public_profile_friend_msg
    _public_profile_friend_msg = ""
    play_sound(click_sound)

    def _on_done(result):
        global _friend_action_pending, _public_profile_friend_msg
        if result and result.get("ok"):
            if result.get("status") == "accepted":
                _friend_action_pending = name
            else:
                _public_profile_friend_msg = get_text("friend_request_sent_msg")
        else:
            err = (result or {}).get("error")
            _public_profile_friend_msg = {
                "already_friends": get_text("already_friends_msg"),
                "target_not_found": get_text("player_not_found"),
            }.get(err, get_text("generic_error_msg"))

    send_friend_request_online(name, on_done=_on_done)


def _remove_friend(name):
    # Entfernen bleibt bewusst sofort/optimistisch (keine Bestätigung durch
    # die Gegenseite nötig) - anders als beim Hinzufügen gibt es hier niemanden,
    # der zustimmen müsste, und es entspricht der bisherigen Instant-Unfriend-UX.
    _remove_friend_local(name)
    play_sound(click_sound)
    remove_friend_online(name)


def public_profile_loop():
    global in_public_profile, _friend_action_pending
    clock = pygame.time.Clock()
    _public_profile_start_fetch()
    while in_public_profile:
        back_rect, friend_rect, message_rect, clan_tag_rect = draw_public_profile()
        if _friend_action_pending is not None:
            _add_friend_local(_friend_action_pending)
            _friend_action_pending = None
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_from_public_profile()
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    back_from_public_profile()
                elif friend_rect is not None and friend_rect.collidepoint(pos):
                    player = (_public_profile_data or {}).get("player", {})
                    name = player.get("name")
                    if name:
                        if name in friends_local:
                            _remove_friend(name)
                        else:
                            _send_friend_request(name)
                elif message_rect is not None and message_rect.collidepoint(pos):
                    player = (_public_profile_data or {}).get("player", {})
                    name = player.get("name")
                    if name:
                        open_chat("dm", name, "in_public_profile")
                elif clan_tag_rect is not None and clan_tag_rect.collidepoint(pos):
                    player = (_public_profile_data or {}).get("player", {})
                    tag = player.get("clan_tag")
                    if tag:
                        open_clan_view(tag, "in_public_profile")
        clock.tick(60)


# =============================
# Freunde-Hub
# =============================

_friends_hub_tab = "roster"  # "roster" | "requests"
_friends_hub_requests = []
_friends_hub_requests_loading = False
_friends_hub_row_rects = []  # [(rect, name)] - Roster-Tab
_friends_hub_request_rects = []  # [(accept_rect, decline_rect, from_name)] - Requests-Tab


def _friends_hub_refresh_requests():
    global _friends_hub_requests_loading
    _friends_hub_requests_loading = True

    def _on_done(result):
        global _friends_hub_requests, _friends_hub_requests_loading
        _friends_hub_requests_loading = False
        _friends_hub_requests = result.get("requests", []) if (result and result.get("ok")) else []

    fetch_incoming_friend_requests(on_done=_on_done)


def _respond_to_request(from_name, accept):
    """Anders als beim Senden einer Anfrage (siehe _send_friend_request()) ist
    hier keine Wartephase nötig - eine eingehende Anfrage anzunehmen erzeugt
    server-seitig immer sofort eine Freundschaft, also ist optimistisches
    lokales Anwenden hier korrekt statt riskant. Läuft ausserdem synchron im
    Main-Thread (direkter Event-Handler-Aufruf), keine Race-Condition-Sorge."""
    global _friends_hub_requests
    play_sound(click_sound)
    _friends_hub_requests = [r for r in _friends_hub_requests if r.get("from_name") != from_name]
    if accept:
        _add_friend_local(from_name)
    respond_friend_request_online(from_name, accept)


def draw_friends_hub():
    global _friends_hub_row_rects, _friends_hub_request_rects
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    _friends_hub_row_rects = []
    _friends_hub_request_rects = []

    title_txt = get_text("friends_hub_title")
    title_s = font.render(title_txt, True, get_text_color())
    shadow_s = font.render(title_txt, True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    # Roster/Requests-Umschalter - gleiches Muster wie Alle/Freunde auf der Rangliste.
    roster_tab_rect = pygame.Rect(60, 65, 160, 36)
    requests_tab_rect = pygame.Rect(226, 65, 160, 36)
    requests_label = get_text("friends_tab_requests")
    if _friends_hub_requests:
        requests_label += f" ({len(_friends_hub_requests)})"
    for rect, active, label in [
        (roster_tab_rect, _friends_hub_tab == "roster", get_text("friends_tab_roster")),
        (requests_tab_rect, _friends_hub_tab == "requests", requests_label),
    ]:
        hov = rect.collidepoint(mouse_pos)
        if active:
            pygame.draw.rect(base_surface, get_button_color(), rect, border_radius=10)
            pygame.draw.rect(base_surface, (100, 160, 255), rect, width=2, border_radius=10)
        else:
            pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), rect, border_radius=10)
            pygame.draw.rect(base_surface, (180, 180, 180), rect, width=1, border_radius=10)
        tab_s = small_font.render(label, True, get_text_color())
        base_surface.blit(tab_s, (rect.centerx - tab_s.get_width() // 2, rect.centery - tab_s.get_height() // 2))

    content_top = 130
    fade_bottom = HEIGHT - 45

    if _friends_hub_tab == "roster":
        if not friends_local:
            empty_s = small_font.render(get_text("no_friends_yet"), True, get_text_color())
            base_surface.blit(empty_s, (WIDTH // 2 - empty_s.get_width() // 2, content_top + 20))
        else:
            y = content_top
            row_h = 44
            for name in sorted(friends_local, key=str.casefold):
                if y + row_h > fade_bottom:
                    break
                row_rect = pygame.Rect(WIDTH // 2 - 220, y, 440, row_h - 6)
                _friends_hub_row_rects.append((pygame.Rect(row_rect), name))
                hov = row_rect.collidepoint(mouse_pos)
                pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), row_rect, border_radius=8)
                pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)
                name_s = menu_font.render(name, True, get_text_color())
                base_surface.blit(name_s, (row_rect.x + 14, row_rect.centery - name_s.get_height() // 2))
                y += row_h
    else:
        if _friends_hub_requests_loading:
            loading_s = small_font.render(get_text("loading"), True, get_text_color())
            base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, content_top + 20))
        elif not _friends_hub_requests:
            empty_s = small_font.render(get_text("no_requests_yet"), True, get_text_color())
            base_surface.blit(empty_s, (WIDTH // 2 - empty_s.get_width() // 2, content_top + 20))
        else:
            y = content_top
            row_h = 48
            for req in _friends_hub_requests:
                if y + row_h > fade_bottom:
                    break
                from_name = req.get("from_name", "")
                row_rect = pygame.Rect(WIDTH // 2 - 260, y, 520, row_h - 6)
                pygame.draw.rect(base_surface, get_panel_color(), row_rect, border_radius=8)
                pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)
                name_s = menu_font.render(from_name, True, get_text_color())
                base_surface.blit(name_s, (row_rect.x + 14, row_rect.centery - name_s.get_height() // 2))

                decline_rect = pygame.Rect(row_rect.right - 96, row_rect.y + 4, 88, row_rect.h - 8)
                accept_rect = pygame.Rect(decline_rect.x - 96, row_rect.y + 4, 88, row_rect.h - 8)
                hov_a = accept_rect.collidepoint(mouse_pos)
                hov_d = decline_rect.collidepoint(mouse_pos)
                pygame.draw.rect(base_surface, (120, 200, 140) if hov_a else (100, 180, 120), accept_rect, border_radius=8)
                pygame.draw.rect(base_surface, (220, 120, 120) if hov_d else (200, 100, 100), decline_rect, border_radius=8)
                a_lbl = small_font.render(get_text("accept_btn"), True, (255, 255, 255))
                d_lbl = small_font.render(get_text("decline_btn"), True, (255, 255, 255))
                base_surface.blit(a_lbl, (accept_rect.centerx - a_lbl.get_width() // 2, accept_rect.centery - a_lbl.get_height() // 2))
                base_surface.blit(d_lbl, (decline_rect.centerx - d_lbl.get_width() // 2, decline_rect.centery - d_lbl.get_height() // 2))
                _friends_hub_request_rects.append((accept_rect, decline_rect, from_name))
                y += row_h

    pygame.display.flip()
    return back_rect, roster_tab_rect, requests_tab_rect


def friends_hub_loop():
    global in_friends_hub, _friends_hub_tab
    clock = pygame.time.Clock()
    _refresh_friends_local()
    _friends_hub_refresh_requests()
    while in_friends_hub:
        back_rect, roster_tab_rect, requests_tab_rect = draw_friends_hub()
        _apply_friends_local_pending()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_to_hub_from_friends_hub()
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    back_to_hub_from_friends_hub()
                elif roster_tab_rect.collidepoint(pos):
                    _friends_hub_tab = "roster"
                    play_sound(click_sound)
                elif requests_tab_rect.collidepoint(pos):
                    _friends_hub_tab = "requests"
                    play_sound(click_sound)
                elif _friends_hub_tab == "roster":
                    for rect, name in _friends_hub_row_rects:
                        if rect.collidepoint(pos):
                            open_public_profile(name, "in_friends_hub")
                            break
                else:
                    for accept_rect, decline_rect, from_name in _friends_hub_request_rects:
                        if accept_rect.collidepoint(pos):
                            _respond_to_request(from_name, True)
                            break
                        elif decline_rect.collidepoint(pos):
                            _respond_to_request(from_name, False)
                            break
        clock.tick(60)


def open_friends_hub_from_hub():
    play_sound(menu_click_sound)
    global in_profile_hub, in_friends_hub, _friends_hub_tab
    in_profile_hub = False
    in_friends_hub = True
    _friends_hub_tab = "roster"


def back_to_hub_from_friends_hub():
    play_sound(click_back_sound)
    global in_friends_hub, in_profile_hub
    in_friends_hub = False
    in_profile_hub = True



# =============================
# Clan-Screens
# =============================

_clan_mine_loading = False
_clan_mine_data = None  # None=lädt, False=Fehler, dict={"in_clan":bool, "tag":..., "name":..., "is_owner":..., "owner_name":..., "members":[...]}
_clan_hub_row_rects = []  # [(rect, name, kick_rect_or_None)]
_clan_hub_msg = ""


def _clan_hub_refresh():
    """Gleiches None/False/dict-Muster wie _public_profile_start_fetch()."""
    global _clan_mine_loading, _clan_mine_data
    _clan_mine_loading = True
    _clan_mine_data = None

    def _on_done(result):
        global _clan_mine_data, _clan_mine_loading
        _clan_mine_loading = False
        _clan_mine_data = result if (result and result.get("ok")) else False

    fetch_my_clan(on_done=_on_done)


def _leave_current_clan():
    global _clan_hub_msg
    play_sound(click_sound)
    _clan_hub_msg = ""

    def _on_done(result):
        global _clan_hub_msg
        if result and result.get("ok"):
            _clan_hub_refresh()
        else:
            _clan_hub_msg = get_text("generic_error_msg")

    leave_clan_online(on_done=_on_done)


def _kick_clan_member(name):
    global _clan_hub_msg
    play_sound(click_sound)
    _clan_hub_msg = ""

    def _on_done(result):
        global _clan_hub_msg
        if result and result.get("ok"):
            _clan_hub_refresh()
        else:
            _clan_hub_msg = get_text("generic_error_msg")

    kick_clan_member_online(name, on_done=_on_done)


def draw_clan_hub():
    global _clan_hub_row_rects
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    _clan_hub_row_rects = []

    title_txt = get_text("clan_hub_title")
    title_s = font.render(title_txt, True, get_text_color())
    shadow_s = font.render(title_txt, True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    create_rect = None
    join_rect = None
    leave_rect = None
    chat_rect = None

    if _clan_mine_loading or _clan_mine_data is None:
        loading_s = font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, HEIGHT // 2 - loading_s.get_height() // 2))
        pygame.display.flip()
        return back_rect, create_rect, join_rect, leave_rect, chat_rect

    in_clan = bool(_clan_mine_data) and _clan_mine_data.get("in_clan")

    if not in_clan:
        empty_s = small_font.render(get_text("clan_not_in_one"), True, get_text_color())
        base_surface.blit(empty_s, (WIDTH // 2 - empty_s.get_width() // 2, 130))

        create_rect = pygame.Rect(WIDTH // 2 - 180, 190, 360, 48)
        join_rect = pygame.Rect(WIDTH // 2 - 180, 250, 360, 48)
        for rect, label, color in [
            (create_rect, get_text("create_clan_btn"), (100, 180, 120)),
            (join_rect, get_text("join_clan_btn"), (100, 150, 220)),
        ]:
            draw_ui_button(base_surface, rect, label, font=menu_font, hovered=rect.collidepoint(mouse_pos),
                            border_color=color, hover_border_color=color, hover_border_width=3)
    else:
        clan = _clan_mine_data
        tag_txt = f"[{clan.get('tag', '')}] {clan.get('name', '')}"
        tag_s = menu_font.render(tag_txt, True, get_text_color())
        base_surface.blit(tag_s, (WIDTH // 2 - tag_s.get_width() // 2, 100))

        members = clan.get("members", [])
        owner_name = clan.get("owner_name")
        count_s = small_font.render(f"{get_text('clan_members_label')}: {len(members)}", True, (150, 150, 170))
        base_surface.blit(count_s, (WIDTH // 2 - count_s.get_width() // 2, 138))

        is_owner = clan.get("is_owner", False)
        content_top = 172
        fade_bottom = HEIGHT - 120
        y = content_top
        row_h = 42
        for m in members:
            if y + row_h > fade_bottom:
                break
            name = m.get("name", "")
            row_rect = pygame.Rect(WIDTH // 2 - 240, y, 480, row_h - 6)
            hov = row_rect.collidepoint(mouse_pos)
            pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), row_rect, border_radius=8)
            pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)
            label_txt = name + (f"  ({get_text('clan_owner_label')})" if name == owner_name else "")
            name_s = small_font.render(label_txt, True, get_text_color())
            base_surface.blit(name_s, (row_rect.x + 12, row_rect.centery - name_s.get_height() // 2))

            kick_rect = None
            if is_owner and name != current_profile_name:
                kick_rect = pygame.Rect(row_rect.right - 82, row_rect.y + 3, 74, row_rect.h - 6)
                hov_k = kick_rect.collidepoint(mouse_pos)
                pygame.draw.rect(base_surface, (220, 120, 120) if hov_k else (200, 100, 100), kick_rect, border_radius=6)
                k_lbl = small_font.render(get_text("kick_btn"), True, (255, 255, 255))
                base_surface.blit(k_lbl, (kick_rect.centerx - k_lbl.get_width() // 2, kick_rect.centery - k_lbl.get_height() // 2))
            _clan_hub_row_rects.append((pygame.Rect(row_rect), name, kick_rect))
            y += row_h

        chat_rect = pygame.Rect(WIDTH // 2 - 240, HEIGHT - 108, 220, 44)
        leave_rect = pygame.Rect(WIDTH // 2 + 20, HEIGHT - 108, 220, 44)
        draw_ui_button(base_surface, chat_rect, get_text("chat_title"), font=small_font,
                        hovered=chat_rect.collidepoint(mouse_pos), radius=10,
                        border_color=(120, 160, 220), hover_border_color=(120, 160, 220), hover_border_width=2)
        draw_ui_button(base_surface, leave_rect, get_text("leave_clan_btn"), font=small_font,
                        hovered=leave_rect.collidepoint(mouse_pos), radius=10,
                        border_color=(200, 100, 100), hover_border_color=(200, 100, 100), hover_border_width=2)

    if _clan_hub_msg:
        msg_s = small_font.render(_clan_hub_msg, True, (220, 60, 60))
        base_surface.blit(msg_s, (WIDTH // 2 - msg_s.get_width() // 2, HEIGHT - 34))

    pygame.display.flip()
    return back_rect, create_rect, join_rect, leave_rect, chat_rect


def clan_hub_loop():
    global in_clan_hub
    clock = pygame.time.Clock()
    _clan_hub_refresh()
    while in_clan_hub:
        back_rect, create_rect, join_rect, leave_rect, chat_rect = draw_clan_hub()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_to_hub_from_clan_hub()
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    back_to_hub_from_clan_hub()
                elif create_rect is not None and create_rect.collidepoint(pos):
                    open_clan_create_from_hub()
                elif join_rect is not None and join_rect.collidepoint(pos):
                    open_clan_search_from_hub()
                elif chat_rect is not None and chat_rect.collidepoint(pos):
                    clan = _clan_mine_data or {}
                    if clan.get("tag"):
                        open_chat("clan", clan["tag"], "in_clan_hub")
                elif leave_rect is not None and leave_rect.collidepoint(pos):
                    _leave_current_clan()
                else:
                    for rect, name, kick_rect in _clan_hub_row_rects:
                        if kick_rect is not None and kick_rect.collidepoint(pos):
                            _kick_clan_member(name)
                            break
                        elif rect.collidepoint(pos):
                            open_public_profile(name, "in_clan_hub")
                            break
        clock.tick(60)


def open_clan_hub_from_hub():
    play_sound(menu_click_sound)
    global in_profile_hub, in_clan_hub, _clan_hub_msg
    in_profile_hub = False
    in_clan_hub = True
    _clan_hub_msg = ""


def back_to_hub_from_clan_hub():
    play_sound(click_back_sound)
    global in_clan_hub, in_profile_hub
    in_clan_hub = False
    in_profile_hub = True


_clan_create_tag = ""
_clan_create_name = ""
_clan_create_msg = ""
_clan_create_busy = False
_clan_create_focus = "tag"  # "tag" | "name"
_clan_create_pending = False  # von _confirm_clan_create()s on_done (Hintergrund-
# Thread) gesetzt, im Main-Thread (clan_create_loop()) angewendet - Navigations-
# Flags (in_X) dürfen nie direkt aus einem Hintergrund-Thread umgeschaltet
# werden, gleiches Prinzip wie bei den anderen _*_pending-Feldern.


def _confirm_clan_create():
    global _clan_create_msg, _clan_create_busy
    tag = _clan_create_tag.strip()
    name = _clan_create_name.strip()
    if not tag or not name:
        _clan_create_msg = get_text("clan_fields_required_msg")
        return
    _clan_create_busy = True
    _clan_create_msg = ""

    def _on_done(result):
        global _clan_create_msg, _clan_create_busy, _clan_create_pending
        _clan_create_busy = False
        if result and result.get("ok"):
            _clan_create_pending = True
        else:
            err = (result or {}).get("error")
            _clan_create_msg = {
                "already_in_clan": get_text("clan_already_in_one_msg"),
                "tag_taken": get_text("clan_tag_taken_msg"),
                "name_taken": get_text("clan_name_taken_msg"),
                "name_filtered": get_text("name_filtered"),
                "unknown_player": get_text("unknown_player_msg"),
            }.get(err, get_text("generic_error_msg"))

    create_clan_online(tag, name, on_done=_on_done)


def draw_clan_create():
    fill_bg(rings=True)
    dark = is_dark_mode()
    pw, ph = 560, 320
    px = WIDTH // 2 - pw // 2
    py = HEIGHT // 2 - ph // 2
    panel = pygame.Rect(px, py, pw, ph)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=18)
    pygame.draw.rect(base_surface, (100, 160, 255) if not dark else (80, 120, 200), panel, width=3, border_radius=18)

    title = menu_font.render(get_text("create_clan_btn"), True, (80, 140, 255) if not dark else (120, 170, 255))
    base_surface.blit(title, (px + pw // 2 - title.get_width() // 2, py + 22))

    mp = get_mouse_pos_logical()
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "

    tag_label_s = small_font.render(get_text("clan_tag_label"), True, get_text_color())
    base_surface.blit(tag_label_s, (px + 40, py + 78))
    tag_box = pygame.Rect(px + 40, py + 100, pw - 80, 44)
    pygame.draw.rect(base_surface, get_bg_color(), tag_box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255) if _clan_create_focus == "tag" else (150, 150, 150), tag_box, width=2, border_radius=10)
    tag_inp = menu_font.render(_clan_create_tag + (cursor if _clan_create_focus == "tag" else ""), True, get_text_color())
    base_surface.blit(tag_inp, (tag_box.x + 10, tag_box.centery - tag_inp.get_height() // 2))

    name_label_s = small_font.render(get_text("clan_name_label"), True, get_text_color())
    base_surface.blit(name_label_s, (px + 40, py + 156))
    name_box = pygame.Rect(px + 40, py + 178, pw - 80, 44)
    pygame.draw.rect(base_surface, get_bg_color(), name_box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255) if _clan_create_focus == "name" else (150, 150, 150), name_box, width=2, border_radius=10)
    name_inp = menu_font.render(_clan_create_name + (cursor if _clan_create_focus == "name" else ""), True, get_text_color())
    base_surface.blit(name_inp, (name_box.x + 10, name_box.centery - name_inp.get_height() // 2))

    if _clan_create_busy:
        msg_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(msg_s, (px + pw // 2 - msg_s.get_width() // 2, py + 230))
    elif _clan_create_msg:
        msg_s = small_font.render(_clan_create_msg, True, (220, 60, 60))
        base_surface.blit(msg_s, (px + pw // 2 - msg_s.get_width() // 2, py + 230))

    confirm_rect = pygame.Rect(px + pw // 2 - 200, py + ph - 62, 180, 44)
    back_rect = pygame.Rect(px + pw // 2 + 20, py + ph - 62, 180, 44)
    hov_c = confirm_rect.collidepoint(mp)
    pygame.draw.rect(base_surface, (60, 180, 100) if not hov_c else (80, 210, 120), confirm_rect, border_radius=12)
    cl = small_font.render(get_text("confirm"), True, (255, 255, 255))
    base_surface.blit(cl, (confirm_rect.centerx - cl.get_width() // 2, confirm_rect.centery - cl.get_height() // 2))
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font, hovered=back_rect.collidepoint(mp))

    pygame.display.flip()
    return tag_box, name_box, confirm_rect, back_rect


def clan_create_loop():
    global in_clan_create, _clan_create_tag, _clan_create_name, _clan_create_focus, _clan_create_pending
    clock = pygame.time.Clock()
    while in_clan_create:
        tag_box, name_box, confirm_rect, back_rect = draw_clan_create()
        if _clan_create_pending:
            _clan_create_pending = False
            back_to_hub_from_clan_create(entered_clan=True)
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_RETURN:
                    _confirm_clan_create()
                elif event.key == pygame.K_TAB:
                    _clan_create_focus = "name" if _clan_create_focus == "tag" else "tag"
                elif event.key == pygame.K_BACKSPACE:
                    if _clan_create_focus == "tag":
                        _clan_create_tag = _clan_create_tag[:-1]
                    else:
                        _clan_create_name = _clan_create_name[:-1]
                elif is_back_key(event.key, allow_backspace=False):
                    play_sound(click_back_sound)
                    back_to_hub_from_clan_create(entered_clan=False)
                else:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126:
                        if _clan_create_focus == "tag" and len(_clan_create_tag) < 5:
                            _clan_create_tag += ch
                        elif _clan_create_focus == "name" and len(_clan_create_name) < 24:
                            _clan_create_name += ch
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if tag_box.collidepoint(pos):
                    _clan_create_focus = "tag"
                elif name_box.collidepoint(pos):
                    _clan_create_focus = "name"
                elif confirm_rect.collidepoint(pos):
                    _confirm_clan_create()
                elif back_rect.collidepoint(pos):
                    play_sound(click_back_sound)
                    back_to_hub_from_clan_create(entered_clan=False)
        clock.tick(60)


def open_clan_create_from_hub():
    play_sound(menu_click_sound)
    global in_clan_hub, in_clan_create, _clan_create_tag, _clan_create_name
    global _clan_create_msg, _clan_create_busy, _clan_create_focus
    in_clan_hub = False
    in_clan_create = True
    _clan_create_tag = ""
    _clan_create_name = ""
    _clan_create_msg = ""
    _clan_create_busy = False
    _clan_create_focus = "tag"


def back_to_hub_from_clan_create(entered_clan):
    global in_clan_create, in_clan_hub
    in_clan_create = False
    in_clan_hub = True
    if entered_clan:
        _clan_hub_refresh()


_clan_search_query = ""
_clan_search_results = None
_clan_search_loading = False
_clan_search_row_rects = []  # [(rect, tag)]
_clan_search_request_id = 0
_clan_search_msg = ""
_clan_join_pending = False  # von _join_clan()s on_done (Hintergrund-Thread)
# gesetzt, im Main-Thread (clan_search_loop()) angewendet - siehe Kommentar
# bei _clan_create_pending.


def _clan_search_trigger(query):
    global _clan_search_loading, _clan_search_request_id, _clan_search_results
    query = query.strip()
    if not query:
        _clan_search_results = []
        _clan_search_loading = False
        return
    _clan_search_loading = True
    _clan_search_request_id += 1
    my_id = _clan_search_request_id

    def _on_done(result):
        global _clan_search_loading, _clan_search_results
        if my_id != _clan_search_request_id:
            return
        _clan_search_loading = False
        _clan_search_results = result.get("results", []) if (result and result.get("ok")) else []

    search_clans_online(query, on_done=_on_done)


def _join_clan(tag):
    global _clan_search_msg
    play_sound(click_sound)
    _clan_search_msg = ""

    def _on_done(result):
        global _clan_search_msg, _clan_join_pending
        if result and result.get("ok"):
            _clan_join_pending = True
        else:
            err = (result or {}).get("error")
            _clan_search_msg = {
                "already_in_clan": get_text("clan_already_in_one_msg"),
                "clan_full": get_text("clan_full_msg"),
                "clan_not_found": get_text("clan_not_found"),
            }.get(err, get_text("generic_error_msg"))

    join_clan_online(tag, on_done=_on_done)


def draw_clan_search():
    global _clan_search_row_rects
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)
    _clan_search_row_rects = []

    title_s = font.render(get_text("clan_search_title"), True, get_text_color())
    shadow_s = font.render(get_text("clan_search_title"), True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    box = pygame.Rect(WIDTH // 2 - 220, 75, 440, 46)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), box, width=2, border_radius=10)
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "
    inp = menu_font.render(_clan_search_query + cursor, True, get_text_color())
    base_surface.blit(inp, (box.x + 12, box.centery - inp.get_height() // 2))

    content_top = 140
    fade_bottom = HEIGHT - 45

    if _clan_search_msg:
        msg_s = small_font.render(_clan_search_msg, True, (220, 60, 60))
        base_surface.blit(msg_s, (WIDTH // 2 - msg_s.get_width() // 2, content_top))
        content_top += 30

    if _clan_search_loading:
        loading_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, content_top + 20))
    elif _clan_search_results is not None and not _clan_search_results:
        empty_s = small_font.render(get_text("clan_not_found"), True, get_text_color())
        base_surface.blit(empty_s, (WIDTH // 2 - empty_s.get_width() // 2, content_top + 20))
    elif _clan_search_results:
        y = content_top
        row_h = 48
        for entry in _clan_search_results:
            if y + row_h > fade_bottom:
                break
            tag = entry.get("tag", "")
            name = entry.get("name", "")
            row_rect = pygame.Rect(WIDTH // 2 - 240, y, 480, row_h - 6)
            _clan_search_row_rects.append((pygame.Rect(row_rect), tag))
            hov = row_rect.collidepoint(mouse_pos)
            pygame.draw.rect(base_surface, get_button_hover_color() if hov else get_panel_color(), row_rect, border_radius=8)
            pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)
            label_txt = f"[{tag}] {name}  ({entry.get('member_count', 0)})"
            name_s = menu_font.render(label_txt, True, get_text_color())
            base_surface.blit(name_s, (row_rect.x + 14, row_rect.centery - name_s.get_height() // 2))
            y += row_h

    pygame.display.flip()
    return back_rect, box


def clan_search_loop():
    global in_clan_search, _clan_search_query, _clan_search_msg, _clan_join_pending
    clock = pygame.time.Clock()
    while in_clan_search:
        back_rect, box = draw_clan_search()
        if _clan_join_pending:
            _clan_join_pending = False
            back_to_hub_from_clan_search(entered_clan=True)
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_BACKSPACE:
                    _clan_search_query = _clan_search_query[:-1]
                    _clan_search_trigger(_clan_search_query)
                elif is_back_key(event.key, allow_backspace=False):
                    play_sound(click_back_sound)
                    back_to_hub_from_clan_search(entered_clan=False)
                else:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(_clan_search_query) < 30:
                        _clan_search_query += ch
                        _clan_search_trigger(_clan_search_query)
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    play_sound(click_back_sound)
                    back_to_hub_from_clan_search(entered_clan=False)
                else:
                    for rect, tag in _clan_search_row_rects:
                        if rect.collidepoint(pos):
                            _join_clan(tag)
                            break
        clock.tick(60)


def open_clan_search_from_hub():
    play_sound(menu_click_sound)
    global in_clan_hub, in_clan_search, _clan_search_query, _clan_search_results, _clan_search_msg
    in_clan_hub = False
    in_clan_search = True
    _clan_search_query = ""
    _clan_search_results = None
    _clan_search_msg = ""


def back_to_hub_from_clan_search(entered_clan):
    global in_clan_search, in_clan_hub
    in_clan_search = False
    in_clan_hub = True
    if entered_clan:
        _clan_hub_refresh()


_clan_view_tag = None
_clan_view_data = None  # None=lädt, False=nicht gefunden, dict=geladen
_clan_view_return_screen = "in_public_profile"
_clan_view_msg = ""
_clan_view_join_pending = False


def _clan_view_start_fetch():
    global _clan_view_data
    _clan_view_data = None
    tag = _clan_view_tag

    def _on_done(result):
        global _clan_view_data
        if tag != _clan_view_tag:
            return  # zwischenzeitlich wurde ein anderer Clan geöffnet
        _clan_view_data = result.get("clan") if (result and result.get("ok")) else False

    fetch_clan_info(tag, on_done=_on_done)


def _join_clan_from_view():
    global _clan_view_msg
    play_sound(click_sound)
    _clan_view_msg = ""

    def _on_done(result):
        global _clan_view_msg, _clan_view_join_pending
        if result and result.get("ok"):
            _clan_view_join_pending = True
        else:
            err = (result or {}).get("error")
            _clan_view_msg = {
                "already_in_clan": get_text("clan_already_in_one_msg"),
                "clan_full": get_text("clan_full_msg"),
            }.get(err, get_text("generic_error_msg"))

    join_clan_online(_clan_view_tag, on_done=_on_done)


def draw_clan_view():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    join_rect = None

    if _clan_view_data is None:
        loading_s = font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (WIDTH // 2 - loading_s.get_width() // 2, HEIGHT // 2 - loading_s.get_height() // 2))
        pygame.display.flip()
        return back_rect, join_rect

    if _clan_view_data is False:
        msg_s = font.render(get_text("clan_not_found"), True, get_text_color())
        base_surface.blit(msg_s, (WIDTH // 2 - msg_s.get_width() // 2, HEIGHT // 2 - msg_s.get_height() // 2))
        pygame.display.flip()
        return back_rect, join_rect

    clan = _clan_view_data
    title_txt = f"[{clan.get('tag', '')}] {clan.get('name', '')}"
    title_s = font.render(title_txt, True, get_text_color())
    shadow_s = font.render(title_txt, True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    owner_name = clan.get("owner_name")
    members = clan.get("members", [])
    count_s = small_font.render(f"{get_text('clan_members_label')}: {len(members)}", True, (150, 150, 170))
    base_surface.blit(count_s, (WIDTH // 2 - count_s.get_width() // 2, 66))

    content_top = 100
    fade_bottom = HEIGHT - 110
    y = content_top
    row_h = 40
    for m in members:
        if y + row_h > fade_bottom:
            break
        name = m.get("name", "")
        row_rect = pygame.Rect(WIDTH // 2 - 220, y, 440, row_h - 6)
        pygame.draw.rect(base_surface, get_panel_color(), row_rect, border_radius=8)
        pygame.draw.rect(base_surface, (180, 180, 180), row_rect, width=1, border_radius=8)
        label_txt = name + (f"  ({get_text('clan_owner_label')})" if name == owner_name else "")
        name_s = small_font.render(label_txt, True, get_text_color())
        base_surface.blit(name_s, (row_rect.x + 12, row_rect.centery - name_s.get_height() // 2))
        y += row_h

    already_in_clan = bool(_clan_mine_data) and _clan_mine_data.get("in_clan")
    if not already_in_clan:
        join_rect = pygame.Rect(WIDTH // 2 - 110, HEIGHT - 90, 220, 44)
        draw_ui_button(base_surface, join_rect, get_text("join_clan_btn"), font=small_font,
                        hovered=join_rect.collidepoint(mouse_pos), radius=10,
                        border_color=(100, 150, 220), hover_border_color=(100, 150, 220), hover_border_width=2)
    if _clan_view_msg:
        msg_s = small_font.render(_clan_view_msg, True, (220, 60, 60))
        base_surface.blit(msg_s, (WIDTH // 2 - msg_s.get_width() // 2, HEIGHT - 34))

    pygame.display.flip()
    return back_rect, join_rect


def clan_view_loop():
    global in_clan_view, _clan_view_join_pending
    clock = pygame.time.Clock()
    _clan_view_start_fetch()
    while in_clan_view:
        back_rect, join_rect = draw_clan_view()
        if _clan_view_join_pending:
            _clan_view_join_pending = False
            _clan_hub_refresh()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_from_clan_view()
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    back_from_clan_view()
                elif join_rect is not None and join_rect.collidepoint(pos):
                    _join_clan_from_view()
        clock.tick(60)


def open_clan_view(tag, return_screen):
    play_sound(menu_click_sound)
    global in_clan_view, _clan_view_tag, _clan_view_return_screen, _clan_view_msg
    global in_public_profile, in_clan_hub, in_clan_search
    in_public_profile = False
    in_clan_hub = False
    in_clan_search = False
    in_clan_view = True
    _clan_view_tag = tag
    _clan_view_return_screen = return_screen
    _clan_view_msg = ""


def back_from_clan_view():
    play_sound(click_back_sound)
    global in_clan_view, in_public_profile, in_clan_hub
    in_clan_view = False
    if _clan_view_return_screen == "in_clan_hub":
        in_clan_hub = True
    else:
        in_public_profile = True



# =============================
# Chat (Freunde-DMs + Clan-Kanal)
# =============================

_chat_conversation_type = None  # "dm" | "clan"
_chat_target = None  # Freundesname oder Clan-Tag
_chat_return_screen = "in_public_profile"
_chat_messages = []
_chat_last_id = 0
_chat_last_poll = 0
_chat_poll_interval_ms = 3000
_chat_input_text = ""
_chat_send_busy = False
_chat_send_pending_poll = False  # von _confirm_send_chat()s on_done (Hintergrund-
# Thread) gesetzt, im Main-Thread (chat_loop()) angewendet - löst einen
# sofortigen Extra-Poll aus, statt die eigene Nachricht optimistisch lokal
# anzuzeigen (siehe Kommentar in _chat_poll() - die Liste bleibt so immer
# exakt gleich dem Server-Stand, ohne eigene Abgleich-Logik).
_chat_msg = ""
_chat_loading = True


def _chat_poll(force=False):
    """Holt neue Nachrichten, höchstens alle _chat_poll_interval_ms (ausser
    force=True, z.B. direkt nach dem eigenen Senden). Reines Polling statt
    Websockets/SSE - dieser Server hat keinerlei Realtime-Infrastruktur
    (einfacher Flask-Dev-Server, siehe server/requirements.txt), Polling
    reiht sich einfach in _online_request_async() ein wie jeder andere
    Screen auch."""
    global _chat_last_poll, _chat_loading
    now = pygame.time.get_ticks()
    if not force and now - _chat_last_poll < _chat_poll_interval_ms:
        return
    _chat_last_poll = now
    conversation_type = _chat_conversation_type
    target = _chat_target
    since_id = _chat_last_id

    def _on_done(result):
        global _chat_messages, _chat_last_id, _chat_loading, _chat_msg
        _chat_loading = False
        if conversation_type != _chat_conversation_type or target != _chat_target:
            return  # zwischenzeitlich wurde ein anderer Chat geöffnet
        if result and result.get("ok"):
            new_msgs = result.get("messages", [])
            if new_msgs:
                _chat_messages = (_chat_messages + new_msgs)[-50:]
                _chat_last_id = max([_chat_last_id] + [m.get("id", 0) for m in new_msgs])
        elif result:
            err = result.get("error")
            _chat_msg = {
                "not_friends": get_text("chat_not_friends_msg"),
                "not_clan_member": get_text("chat_not_clan_member_msg"),
                "target_not_found": get_text("player_not_found"),
            }.get(err, get_text("generic_error_msg"))

    fetch_chat_messages(conversation_type, target, since_id, on_done=_on_done)


def _confirm_send_chat():
    global _chat_input_text, _chat_send_busy, _chat_msg
    text = _chat_input_text.strip()
    if not text:
        return
    _chat_input_text = ""
    _chat_send_busy = True
    _chat_msg = ""
    conversation_type = _chat_conversation_type
    target = _chat_target

    def _on_done(result):
        global _chat_send_busy, _chat_msg, _chat_send_pending_poll
        _chat_send_busy = False
        if conversation_type != _chat_conversation_type or target != _chat_target:
            return
        if result and result.get("ok"):
            _chat_send_pending_poll = True
        else:
            err = (result or {}).get("error")
            _chat_msg = {
                "message_too_long": get_text("chat_message_too_long_msg"),
                "message_filtered": get_text("chat_message_filtered_msg"),
                "not_friends": get_text("chat_not_friends_msg"),
                "not_clan_member": get_text("chat_not_clan_member_msg"),
                "rate_limited": get_text("chat_rate_limited_msg"),
            }.get(err, get_text("generic_error_msg"))

    send_chat_message_online(conversation_type, target, text, on_done=_on_done)


def draw_chat():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    target_label = f"[{_chat_target}]" if _chat_conversation_type == "clan" else (_chat_target or "")
    title_txt = f"{get_text('chat_title')} - {target_label}"
    title_s = font.render(title_txt, True, get_text_color())
    shadow_s = font.render(title_txt, True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font,
                    hovered=back_rect.collidepoint(mouse_pos), radius=10)

    panel = pygame.Rect(60, 70, WIDTH - 120, HEIGHT - 220)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=12)
    pygame.draw.rect(base_surface, (180, 180, 180), panel, width=1, border_radius=12)

    if _chat_loading and not _chat_messages:
        loading_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(loading_s, (panel.centerx - loading_s.get_width() // 2, panel.centery - loading_s.get_height() // 2))
    elif not _chat_messages:
        empty_s = small_font.render(get_text("no_messages_yet"), True, get_text_color())
        base_surface.blit(empty_s, (panel.centerx - empty_s.get_width() // 2, panel.centery - empty_s.get_height() // 2))
    else:
        # Neueste Nachricht unten (übliche Chat-Konvention) - von unten nach
        # oben zeichnen, abbrechen sobald der Platz im Panel aufgebraucht ist.
        y = panel.bottom - 12
        for msg in reversed(_chat_messages):
            sender = msg.get("sender_name", "")
            body = msg.get("body", "")
            is_me = sender == current_profile_name
            line_s = small_font.render(f"{sender}: {body}", True, (120, 170, 255) if is_me else get_text_color())
            line_h = line_s.get_height() + 6
            if y - line_h < panel.top + 10:
                break
            y -= line_h
            base_surface.blit(line_s, (panel.x + 14, y))

    input_box = pygame.Rect(60, panel.bottom + 14, WIDTH - 220, 46)
    pygame.draw.rect(base_surface, get_bg_color(), input_box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), input_box, width=2, border_radius=10)
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "
    visible = _fit_text_tail(_chat_input_text, small_font, input_box.w - 20)
    inp_s = small_font.render(visible + cursor, True, get_text_color())
    base_surface.blit(inp_s, (input_box.x + 10, input_box.centery - inp_s.get_height() // 2))

    send_rect = pygame.Rect(input_box.right + 12, input_box.y, 90, 46)
    hov_send = send_rect.collidepoint(mouse_pos)
    pygame.draw.rect(base_surface, (80, 210, 120) if hov_send else (60, 180, 100), send_rect, border_radius=10)
    send_lbl = small_font.render(get_text("send_btn"), True, (255, 255, 255))
    base_surface.blit(send_lbl, (send_rect.centerx - send_lbl.get_width() // 2, send_rect.centery - send_lbl.get_height() // 2))

    if _chat_msg:
        msg_s = small_font.render(_chat_msg, True, (220, 60, 60))
        base_surface.blit(msg_s, (WIDTH // 2 - msg_s.get_width() // 2, input_box.bottom + 8))

    pygame.display.flip()
    return back_rect, input_box, send_rect


def chat_loop():
    global in_chat, _chat_input_text, _chat_send_pending_poll
    clock = pygame.time.Clock()
    while in_chat:
        back_rect, input_box, send_rect = draw_chat()
        if _chat_send_pending_poll:
            _chat_send_pending_poll = False
            _chat_poll(force=True)
        _chat_poll()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_RETURN:
                    _confirm_send_chat()
                elif event.key == pygame.K_BACKSPACE:
                    _chat_input_text = _chat_input_text[:-1]
                elif event.key == pygame.K_v and (pygame.key.get_mods() & pygame.KMOD_CTRL):
                    pasted = _paste_into_field(300)
                    if pasted is not None:
                        _chat_input_text = pasted
                elif is_back_key(event.key, allow_backspace=False):
                    play_sound(click_back_sound)
                    back_from_chat()
                else:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(_chat_input_text) < 300:
                        _chat_input_text += ch
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if back_rect.collidepoint(pos):
                    play_sound(click_back_sound)
                    back_from_chat()
                elif send_rect.collidepoint(pos):
                    _confirm_send_chat()
        clock.tick(60)


def open_chat(conversation_type, target, return_screen):
    play_sound(menu_click_sound)
    global in_chat, _chat_conversation_type, _chat_target, _chat_return_screen
    global _chat_messages, _chat_last_id, _chat_last_poll, _chat_input_text, _chat_msg, _chat_loading
    global in_public_profile, in_clan_hub
    in_public_profile = False
    in_clan_hub = False
    in_chat = True
    _chat_conversation_type = conversation_type
    _chat_target = target
    _chat_return_screen = return_screen
    _chat_messages = []
    _chat_last_id = 0
    _chat_last_poll = 0
    _chat_input_text = ""
    _chat_msg = ""
    _chat_loading = True


def back_from_chat():
    play_sound(click_back_sound)
    global in_chat, in_public_profile, in_clan_hub
    in_chat = False
    if _chat_return_screen == "in_clan_hub":
        in_clan_hub = True
    else:
        in_public_profile = True


def _get_clipboard_text():
    """Liest Text aus der System-Zwischenablage, best-effort - gibt None
    zurück, wenn nichts verfügbar ist (z.B. Zwischenablage leer/kein Text
    drin, oder pygame.scrap auf dieser Plattform nicht verfügbar). pygame
    unterstützt Ctrl+V nicht automatisch in Textfeldern, daher dieser Helfer
    für die Paste-Buttons bei Redeem-Key/-Code."""
    text = _get_clipboard_text_scrap()
    if text:
        return text
    if sys.platform.startswith("linux"):
        text = _get_clipboard_text_linux_cli()
        if text:
            return text
    return None


def _get_clipboard_text_scrap():
    try:
        if not pygame.scrap.get_init():
            pygame.scrap.init()
        raw = pygame.scrap.get(pygame.SCRAP_TEXT)
        if not raw:
            return None
        text = raw.decode("utf-8", errors="ignore").split("\x00")[0]
        return text.strip() or None
    except Exception:
        return None


def _get_clipboard_text_linux_cli():
    """Fallback für Linux, wo pygame.scrap oft nicht zuverlässig funktioniert -
    v.a. unter Wayland, das SDLs X11-Selection-basiertes Clipboard-Backend
    (worauf pygame.scrap aufbaut) nicht unterstützt, ausser über XWayland.
    Probiert der Reihe nach gängige CLI-Tools durch, je nachdem was auf dem
    jeweiligen System installiert ist (wl-clipboard unter Wayland, xclip/xsel
    unter X11) - fehlt ein Tool, wird einfach das nächste versucht."""
    for cmd in (["wl-paste", "--no-newline"], ["xclip", "-selection", "clipboard", "-o"],
                ["xsel", "--clipboard", "--output"]):
        try:
            result = subprocess.run(cmd, capture_output=True, timeout=2)
        except Exception:
            continue
        if result.returncode == 0 and result.stdout:
            text = result.stdout.decode("utf-8", errors="ignore").strip()
            if text:
                return text
    return None


def _paste_into_field(max_len):
    """Zwischenablage-Text für ein Eingabefeld, gefiltert auf druckbares ASCII
    (gleiche Zeichenmenge wie beim normalen Eintippen) und auf max_len gekürzt.
    Ersetzt den Feldinhalt komplett (nicht anhängen) - None falls nichts in
    der Zwischenablage steht."""
    clip = _get_clipboard_text()
    if not clip:
        return None
    filtered = "".join(c for c in clip if 32 <= ord(c) <= 126)
    return filtered[:max_len] if filtered else None


def _fit_text_tail(text, font_obj, max_w):
    """Gibt das Suffix von text zurück, das in max_w Pixel passt - für lange
    Eingabefelder (99-Zeichen-Key), zeigt immer das Ende beim Cursor."""
    for i in range(len(text)):
        if font_obj.size(text[i:])[0] <= max_w:
            return text[i:]
    return text


_redeem_code_pending = None  # (code, coins) von _on_done (Hintergrund-Thread)
# gesetzt, im Main-Thread (redeem_code_loop()) angewendet - siehe Kommentar
# bei _rename_pending_local_name, gleiche Race-Condition-Vorsicht. Nur für
# normale Codes (Coins) - Verified-Keys brauchen keine lokale Buchführung.


def _confirm_redeem_code():
    """Ein Eingabefeld für beides: Verified-Keys sind immer exakt 99 Zeichen
    (siehe VERIFIED_KEY_LENGTH in server/db.py), alles andere ist ein
    normaler Redeem-Code - daran unterscheidet der Client, welchen der beiden
    Endpunkte er anspricht, ohne dass der Spieler das explizit wählen muss."""
    global _redeem_code_msg, _redeem_code_busy
    text = _redeem_code_text.strip()
    if not text:
        _redeem_code_msg = get_text("code_invalid_msg")
        return

    if len(text) == 99:
        _redeem_code_busy = True
        _redeem_code_msg = ""

        def _on_done_key(result):
            global _redeem_code_msg, _redeem_code_busy
            _redeem_code_busy = False
            if result and result.get("ok"):
                _redeem_code_msg = get_text("redeem_key_success")
            else:
                err = (result or {}).get("error")
                _redeem_code_msg = {
                    "invalid_key": get_text("key_invalid_msg"),
                    "unknown_player": get_text("unknown_player_msg"),
                }.get(err, get_text("generic_error_msg"))

        redeem_verified_key_online(text, on_done=_on_done_key)
        return

    code = text.upper()
    if code in redeemed_codes_local:
        # Schon lokal als eingelöst bekannt - kein Serverping nötig, direkt
        # dieselbe Meldung wie der Server bei einem Doppel-Versuch.
        _redeem_code_msg = get_text("code_already_redeemed_msg")
        return
    _redeem_code_busy = True
    _redeem_code_msg = ""

    def _on_done_code(result):
        global _redeem_code_msg, _redeem_code_busy, _redeem_code_pending
        _redeem_code_busy = False
        if result and result.get("ok"):
            gained = result.get("coins", 0)
            _redeem_code_pending = (code, gained)
            _redeem_code_msg = f"{get_text('redeem_code_success')} (+{gained} {get_text('coins')})"
        else:
            err = (result or {}).get("error")
            _redeem_code_msg = {
                "invalid_code": get_text("code_invalid_msg"),
                "already_redeemed": get_text("code_already_redeemed_msg"),
                "unknown_player": get_text("unknown_player_msg"),
            }.get(err, get_text("generic_error_msg"))

    redeem_code_online(code, on_done=_on_done_code)


def draw_redeem_code():
    fill_bg(rings=True)
    dark = is_dark_mode()
    pw, ph = 600, 260
    px = WIDTH // 2 - pw // 2
    py = HEIGHT // 2 - ph // 2
    panel = pygame.Rect(px, py, pw, ph)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=18)
    pygame.draw.rect(base_surface, (100, 160, 255) if not dark else (80, 120, 200), panel, width=3, border_radius=18)

    title = menu_font.render(get_text("redeem_code_btn"), True, (80, 140, 255) if not dark else (120, 170, 255))
    base_surface.blit(title, (px + pw // 2 - title.get_width() // 2, py + 24))

    box = pygame.Rect(px + 30, py + 90, pw - 60, 46)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), box, width=2, border_radius=10)
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "
    # _fit_text_tail statt direktem Render, da ein Verified-Key (99 Zeichen)
    # sonst weit über den Kasten hinaus liefe - zeigt immer das Ende beim Cursor.
    visible = _fit_text_tail(_redeem_code_text, small_font, box.w - 20)
    inp = small_font.render(visible + cursor, True, get_text_color())
    base_surface.blit(inp, (box.x + 10, box.centery - inp.get_height() // 2))

    if _redeem_code_busy:
        msg_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(msg_s, (px + pw // 2 - msg_s.get_width() // 2, py + 150))
    elif _redeem_code_msg:
        col = (60, 190, 100) if (_redeem_code_msg.startswith(get_text("redeem_code_success")) or _redeem_code_msg == get_text("redeem_key_success")) else (220, 60, 60)
        msg_s = small_font.render(_redeem_code_msg, True, col)
        base_surface.blit(msg_s, (px + pw // 2 - msg_s.get_width() // 2, py + 150))

    paste_rect = pygame.Rect(px + 30, py + ph - 62, 170, 44)
    confirm_rect = pygame.Rect(px + 215, py + ph - 62, 170, 44)
    back_rect = pygame.Rect(px + 400, py + ph - 62, 170, 44)
    mp = get_mouse_pos_logical()
    draw_ui_button(base_surface, paste_rect, get_text("paste_btn"), font=small_font, hovered=paste_rect.collidepoint(mp))
    hov_c = confirm_rect.collidepoint(mp)
    pygame.draw.rect(base_surface, (60, 180, 100) if not hov_c else (80, 210, 120), confirm_rect, border_radius=12)
    cl = small_font.render(get_text("confirm"), True, (255, 255, 255))
    base_surface.blit(cl, (confirm_rect.centerx - cl.get_width() // 2, confirm_rect.centery - cl.get_height() // 2))
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font, hovered=back_rect.collidepoint(mp))

    pygame.display.flip()
    return paste_rect, confirm_rect, back_rect


def redeem_code_loop():
    global in_redeem_code, in_more_menu, _redeem_code_text, _redeem_code_pending, redeemed_codes_local
    clock = pygame.time.Clock()
    while in_redeem_code:
        paste_rect, confirm_rect, back_rect = draw_redeem_code()
        if _redeem_code_pending is not None:
            pending_code, pending_coins = _redeem_code_pending
            stats["coins"] = stats.get("coins", 0) + pending_coins
            save_section("stats", stats)
            if pending_code not in redeemed_codes_local:
                redeemed_codes_local.append(pending_code)
                save_current_profile()
            _redeem_code_pending = None
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_RETURN:
                    _confirm_redeem_code()
                elif event.key == pygame.K_BACKSPACE:
                    _redeem_code_text = _redeem_code_text[:-1]
                elif event.key == pygame.K_v and (pygame.key.get_mods() & pygame.KMOD_CTRL):
                    pasted = _paste_into_field(99)
                    if pasted is not None:
                        _redeem_code_text = pasted
                elif is_back_key(event.key, allow_backspace=False):
                    play_sound(click_back_sound)
                    in_redeem_code = False
                    in_more_menu = True
                else:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(_redeem_code_text) < 99:
                        _redeem_code_text += ch
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if paste_rect.collidepoint(pos):
                    pasted = _paste_into_field(99)
                    if pasted is not None:
                        _redeem_code_text = pasted
                    play_sound(click_sound)
                elif confirm_rect.collidepoint(pos):
                    _confirm_redeem_code()
                elif back_rect.collidepoint(pos):
                    play_sound(click_back_sound)
                    in_redeem_code = False
                    in_more_menu = True
        clock.tick(60)


_rename_pending_local_name = None  # von _on_done (Hintergrund-Thread) gesetzt,
# vom Main-Thread in rename_profile_loop() angewendet - rename_current_profile_local()
# schreibt die Save-Datei, das darf nicht aus einem Hintergrund-Thread passieren
# (Race Condition mit anderen Speichervorgängen im Main-Thread).


def _confirm_rename():
    global _rename_msg, _rename_busy
    new_name = _rename_text.strip()
    if not new_name:
        _rename_msg = get_text("name_empty")
        return
    if not is_name_allowed(new_name):
        _rename_msg = get_text("name_filtered")
        return
    _rename_busy = True
    _rename_msg = ""

    def _on_done(result):
        global _rename_msg, _rename_busy, _rename_pending_local_name
        _rename_busy = False
        if result and result.get("ok"):
            _rename_pending_local_name = result.get("name", new_name)
            _rename_msg = get_text("rename_success")
        else:
            err = (result or {}).get("error")
            _rename_msg = {
                "name_taken": get_text("name_taken"),
                "name_filtered": get_text("name_filtered"),
                "unknown_player": get_text("unknown_player_msg"),
            }.get(err, get_text("generic_error_msg"))

    rename_profile_online(new_name, on_done=_on_done)


def draw_rename_profile():
    fill_bg(rings=True)
    dark = is_dark_mode()
    pw, ph = 520, 260
    px = WIDTH // 2 - pw // 2
    py = HEIGHT // 2 - ph // 2
    panel = pygame.Rect(px, py, pw, ph)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=18)
    pygame.draw.rect(base_surface, (100, 160, 255) if not dark else (80, 120, 200), panel, width=3, border_radius=18)

    title = menu_font.render(get_text("rename_btn"), True, (80, 140, 255) if not dark else (120, 170, 255))
    base_surface.blit(title, (px + pw // 2 - title.get_width() // 2, py + 24))

    box = pygame.Rect(px + 40, py + 90, pw - 80, 46)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=10)
    pygame.draw.rect(base_surface, (120, 180, 255), box, width=2, border_radius=10)
    cursor = "|" if (pygame.time.get_ticks() // 500) % 2 == 0 else " "
    inp = menu_font.render(_rename_text + cursor, True, get_text_color())
    base_surface.blit(inp, (box.x + 10, box.centery - inp.get_height() // 2))

    if _rename_busy:
        msg_s = small_font.render(get_text("loading"), True, get_text_color())
        base_surface.blit(msg_s, (px + pw // 2 - msg_s.get_width() // 2, py + 150))
    elif _rename_msg:
        col = (60, 190, 100) if _rename_msg == get_text("rename_success") else (220, 60, 60)
        msg_s = small_font.render(_rename_msg, True, col)
        base_surface.blit(msg_s, (px + pw // 2 - msg_s.get_width() // 2, py + 150))

    confirm_rect = pygame.Rect(px + pw // 2 - 200, py + ph - 62, 180, 44)
    back_rect = pygame.Rect(px + pw // 2 + 20, py + ph - 62, 180, 44)
    mp = get_mouse_pos_logical()
    hov_c = confirm_rect.collidepoint(mp)
    pygame.draw.rect(base_surface, (60, 180, 100) if not hov_c else (80, 210, 120), confirm_rect, border_radius=12)
    cl = small_font.render(get_text("confirm"), True, (255, 255, 255))
    base_surface.blit(cl, (confirm_rect.centerx - cl.get_width() // 2, confirm_rect.centery - cl.get_height() // 2))
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font, hovered=back_rect.collidepoint(mp))

    pygame.display.flip()
    return confirm_rect, back_rect


def rename_profile_loop():
    global in_rename_profile, in_profile_hub, _rename_text, _rename_pending_local_name
    clock = pygame.time.Clock()
    while in_rename_profile:
        confirm_rect, back_rect = draw_rename_profile()
        if _rename_pending_local_name is not None:
            rename_current_profile_local(_rename_pending_local_name)
            _rename_pending_local_name = None
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_RETURN:
                    _confirm_rename()
                elif event.key == pygame.K_BACKSPACE:
                    _rename_text = _rename_text[:-1]
                elif is_back_key(event.key, allow_backspace=False):
                    play_sound(click_back_sound)
                    in_rename_profile = False
                    in_profile_hub = True
                else:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(_rename_text) < 20:
                        _rename_text += ch
            elif event.type == pygame.MOUSEBUTTONDOWN:
                pos = get_mouse_pos_logical()
                if confirm_rect.collidepoint(pos):
                    _confirm_rename()
                elif back_rect.collidepoint(pos):
                    play_sound(click_back_sound)
                    in_rename_profile = False
                    in_profile_hub = True
        clock.tick(60)


_ach_scroll = 0
_ach_row_rects = []   # list of (rect, code) — populated each draw call
_ach_detail_code = None

def draw_achievements_screen():
    import math as _math
    global _ach_scroll, _ach_row_rects
    _ach_row_rects = []
    mouse_pos = get_mouse_pos_logical()
    dark = is_dark_mode()
    fill_bg(rings=True)

    # Title
    title_s = font.render(get_text("achievements"), True, get_text_color())
    shadow_s = font.render(get_text("achievements"), True, (100, 100, 120))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s, (WIDTH // 2 - title_s.get_width() // 2, 20))

    # Coin display
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    coin_txt = small_font.render(f"{get_text('coins')}: {stats.get('coins', 0)}", True, get_text_color())
    base_surface.blit(coin_txt, (25, 22))

    ACHIEVEMENT_CATS = [
        ("🎯 Score",     ["score_10", "score_50", "score_100", "score_200", "score_500", "score_1000"]),
        ("🔁 Runden",    ["first_round", "five_rounds", "ten_rounds", "twenty_rounds", "fifty_rounds"]),
        ("🔥 Hardcore",  ["hardcore_start", "hardcore_survivor"]),
        ("Unmöglich", ["impossible_start", "impossible_survivor"]),
        ("💚 Pickups",   ["pickup_1up", "pickup_3x_1up", "pickup_10x_1up"]),
        ("💰 Coins",     ["coins_100", "coins_500", "coins_1000"]),
        ("🏆 Bonus",     ["no_damage_run", "speed_demon"]),
    ]
    _ach_total_rows = sum(1 for _, codes in ACHIEVEMENT_CATS for c in codes if c in achievement_definitions)
    _ach_nav_total = _ach_total_rows + 1  # + Back-Button

    # Back button (top-right)
    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
    hov = back_rect.collidepoint(mouse_pos) or nav_is_focused("achievements", _ach_total_rows, _ach_nav_total)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font, hovered=hov, radius=10)

    content_top = 75
    fade_top = content_top + 18
    fade_bottom = HEIGHT - 45
    fade_dist = 28

    total_unlocked = sum(1 for _, codes in ACHIEVEMENT_CATS for c in codes if achievements.get(c, False))
    total_count = sum(len(codes) for _, codes in ACHIEVEMENT_CATS)
    prog_s = small_font.render(f"{total_unlocked}/{total_count}", True, (180, 210, 120) if dark else (60, 120, 60))
    base_surface.blit(prog_s, (WIDTH // 2 - prog_s.get_width() // 2, content_top - 20))

    y = content_top - _ach_scroll
    clip_rect = pygame.Rect(0, fade_top, WIDTH, fade_bottom - fade_top)
    base_surface.set_clip(clip_rect)

    for cat_label, codes in ACHIEVEMENT_CATS:
        unlocked_count = sum(1 for c in codes if achievements.get(c, False))
        all_done = unlocked_count == len(codes)
        hdr_col = (255, 210, 50) if all_done else ((160, 160, 210) if dark else (80, 80, 140))
        hdr_s = small_font.render(f"{cat_label}  {unlocked_count}/{len(codes)}", True, hdr_col)
        if fade_top <= y + 24 and y < fade_bottom:
            base_surface.blit(hdr_s, (55, y + 3))
        y += 28

        for code in codes:
            if code not in achievement_definitions:
                continue
            unlocked = achievements.get(code, False)
            title_txt = get_achievement_name(code)
            _, _, coins = achievement_definitions[code]

            # Rect+Registrierung unconditional (auch außerhalb des sichtbaren Bereichs),
            # damit Tastatur-Fokus auch nicht gerenderte Zeilen erreichen kann.
            ach_rect = pygame.Rect(55, y, WIDTH - 110, 32)
            _ach_row_idx = len(_ach_row_rects)
            _ach_row_rects.append((pygame.Rect(ach_rect), code))

            if y + 32 > fade_top and y < fade_bottom:
                if unlocked:
                    bg_col = (28, 78, 48) if dark else (210, 240, 220)
                    border_col = (70, 190, 110)
                    text_col = (190, 255, 210) if dark else (20, 100, 50)
                    prefix = "✔ "
                else:
                    bg_col = (38, 38, 52) if dark else (220, 220, 228)
                    border_col = (65, 65, 88) if dark else (180, 180, 200)
                    text_col = (95, 95, 125) if dark else (140, 140, 160)
                    prefix = ""

                hov_row = (ach_rect.collidepoint(mouse_pos) or nav_is_focused("achievements", _ach_row_idx, _ach_nav_total)) and _ach_detail_code is None
                if hov_row:
                    hover_bg = (45, 95, 65) if dark else (195, 230, 210)
                    pygame.draw.rect(base_surface, hover_bg, ach_rect, border_radius=7)
                else:
                    pygame.draw.rect(base_surface, bg_col, ach_rect, border_radius=7)
                pygame.draw.rect(base_surface, border_col, ach_rect, width=2, border_radius=7)

                if unlocked:
                    gv = int(160 + 60 * _math.sin(time.time() * 3 + codes.index(code) * 0.8))
                    pygame.draw.rect(base_surface, (gv, int(gv * 0.85), int(gv * 0.3)), ach_rect, width=1, border_radius=7)

                row_txt = small_font.render(f"{prefix}{title_txt}", True, text_col)
                base_surface.blit(row_txt, (63, y + 7))

                if coins > 0:
                    coin_lbl = small_font.render(f"+{coins}", True, (255, 210, 50) if unlocked else (120, 100, 40))
                    base_surface.blit(coin_lbl, (ach_rect.right - coin_lbl.get_width() - 10, y + 7))

            y += 36
        y += 10

    base_surface.set_clip(None)

    # Scrollbar
    total_h = y + _ach_scroll - content_top
    visible_h = fade_bottom - fade_top
    if total_h > visible_h:
        sb_h = max(30, int(visible_h * visible_h / total_h))
        sb_y = fade_top + int(_ach_scroll / max(1, total_h - visible_h) * (visible_h - sb_h))
        pygame.draw.rect(base_surface, (120, 120, 150), pygame.Rect(WIDTH - 10, sb_y, 6, sb_h), border_radius=3)

    # Footer hint
    hint_txt = "ESC: Zurück" if _ach_detail_code is None else "Klick / ESC: Schließen"
    hint = small_font.render(hint_txt, True, (120, 120, 140))
    base_surface.blit(hint, (WIDTH // 2 - hint.get_width() // 2, HEIGHT - 22))

    # Detail popup
    if _ach_detail_code and _ach_detail_code in achievement_definitions:
        _draw_ach_detail_popup(_ach_detail_code)

    draw_achievement_popups(base_surface)
    pygame.display.flip()
    return back_rect


def _draw_ach_detail_popup(code):
    import math as _math
    dark = is_dark_mode()
    unlocked = achievements.get(code, False)
    title_txt = get_achievement_name(code)
    desc_txt = get_achievement_description(code)
    _, _, coins = achievement_definitions[code]

    pw, ph = 360, 160
    px = WIDTH // 2 - pw // 2
    py = HEIGHT // 2 - ph // 2

    overlay = pygame.Surface((WIDTH, HEIGHT), pygame.SRCALPHA)
    overlay.fill((0, 0, 0, 140))
    base_surface.blit(overlay, (0, 0))

    panel_col = (25, 20, 48) if dark else (245, 245, 255)
    pygame.draw.rect(base_surface, panel_col, pygame.Rect(px, py, pw, ph), border_radius=14)

    gv = int(180 + 55 * _math.sin(time.time() * 4))
    border_col = (gv, int(gv * 0.85), 40) if unlocked else (80, 80, 110)
    pygame.draw.rect(base_surface, border_col, pygame.Rect(px, py, pw, ph), width=2, border_radius=14)

    status_col = (80, 220, 120) if unlocked else (160, 100, 100)
    status_txt = "✔ Freigeschaltet" if unlocked else "Gesperrt"
    st_s = small_font.render(status_txt, True, status_col)
    base_surface.blit(st_s, (px + pw // 2 - st_s.get_width() // 2, py + 10))

    title_s = menu_font.render(title_txt, True, get_text_color())
    base_surface.blit(title_s, (px + pw // 2 - title_s.get_width() // 2, py + 34))

    if desc_txt:
        words = desc_txt.split()
        lines = []
        line = ""
        for w in words:
            test = (line + " " + w).strip()
            if small_font.size(test)[0] <= pw - 30:
                line = test
            else:
                lines.append(line)
                line = w
        if line:
            lines.append(line)
        desc_col = (180, 180, 200) if dark else (80, 80, 100)
        for i, l in enumerate(lines[:3]):
            ls = small_font.render(l, True, desc_col)
            base_surface.blit(ls, (px + pw // 2 - ls.get_width() // 2, py + 80 + i * 20))

    if coins > 0:
        coin_col = (255, 210, 50) if unlocked else (120, 100, 40)
        coin_s = small_font.render(f"Belohnung: +{coins} Coins", True, coin_col)
        base_surface.blit(coin_s, (px + pw // 2 - coin_s.get_width() // 2, py + ph - 28))


_ctrl_active_tab = 0  # 0=Standard, 1=2-Player, 2=Menü-Navigation

def draw_controls_menu():
    global _ctrl_listening, _ctrl_scroll
    mouse_pos = get_mouse_pos_logical()
    dark = is_dark_mode()
    fill_bg(rings=True)

    # ── dark overlay like Minecraft's pause screen ──
    overlay = pygame.Surface((WIDTH, HEIGHT), pygame.SRCALPHA)
    overlay.fill((0, 0, 0, 120))
    base_surface.blit(overlay, (0, 0))

    # Title
    title_s = font.render(get_text("controls_title"), True, (255, 255, 255))
    shadow_s = font.render(get_text("controls_title"), True, (80, 80, 80))
    base_surface.blit(shadow_s, (WIDTH // 2 - title_s.get_width() // 2 + 2, 22))
    base_surface.blit(title_s,  (WIDTH // 2 - title_s.get_width() // 2,     20))

    hint = small_font.render(get_text("controls_hint_legend"), True, (180, 180, 200))
    base_surface.blit(hint, (WIDTH // 2 - hint.get_width() // 2, 72))

    # ── Tabs: Standard / 2-Player / Menü-Navigation - jede Aktion lebt nur in EINEM Tab,
    # 2P-Tasten waren vorher in derselben flachen Liste wie Standard-Tasten vermischt. ──
    _ctrl_tab_defs = [
        (get_text("controls_tab_standard"), _STANDARD_ACTIONS),
        (get_text("controls_tab_2p"), _2P_ACTIONS),
        (get_text("controls_tab_menu"), _MENU_NAV_ACTIONS),
    ]
    _tab_w, _tab_h, _tab_gap = 160, 34, 8
    _tabs_total_w = len(_ctrl_tab_defs) * _tab_w + (len(_ctrl_tab_defs) - 1) * _tab_gap
    _tabs_x0 = WIDTH // 2 - _tabs_total_w // 2
    _tabs_y = 96
    _ctrl_tab_rects = []
    for _ti, (_tlabel, _tactions) in enumerate(_ctrl_tab_defs):
        _trect = pygame.Rect(_tabs_x0 + _ti * (_tab_w + _tab_gap), _tabs_y, _tab_w, _tab_h)
        _ctrl_tab_rects.append(_trect)
        _tactive = (_ti == _ctrl_active_tab)
        _thov = _trect.collidepoint(mouse_pos)
        _tbg = (70, 120, 200) if _tactive else (get_button_hover_color() if _thov else get_button_color())
        pygame.draw.rect(base_surface, _tbg, _trect, border_radius=8)
        pygame.draw.rect(base_surface, (150, 180, 220) if _tactive else (150, 150, 150), _trect, width=2, border_radius=8)
        _tlbl_s = small_font.render(_tlabel, True, (255, 255, 255) if _tactive else get_text_color())
        base_surface.blit(_tlbl_s, (_trect.centerx - _tlbl_s.get_width() // 2, _trect.centery - _tlbl_s.get_height() // 2))

    kb = get_keybindings()
    menu_nav_extras = get_menu_nav_extra_keys()
    actions = _ctrl_tab_defs[_ctrl_active_tab][1]
    is_menu_nav_tab = (_ctrl_active_tab == 2)

    # Konflikt-Erkennung: welche Tasten sind bei mehr als einer Action GEBUNDEN
    # INNERHALB DES AKTIVEN TABS? (nur für normale Gameplay-Keybindings - die
    # Menü-Navigation lebt in einem eigenen Speicher und Pfeiltasten/WASD dort
    # sollen nicht als "Konflikt" mit Jump/2P markiert werden). Bewusst nur über
    # `actions` (aktiver Tab) statt kb.items() (alle Tabs) - eine Taste, die z.B.
    # in Standard UND 2-Player gebunden ist, soll in keinem der beiden Tabs als
    # Konflikt aufleuchten, nur eine Doppelbelegung INNERHALB desselben Tabs.
    _key_use_count = {}
    if not is_menu_nav_tab:
        for _a in actions:
            for _k in kb.get(_a, []):
                _key_use_count[_k] = _key_use_count.get(_k, 0) + 1

    table_w   = min(760, WIDTH - 60)
    table_x   = WIDTH // 2 - table_w // 2
    row_h     = 54
    rows_y0   = 142
    divider_c = (70, 72, 90) if dark else (190, 192, 205)

    # ── Scroll-Viewport: Liste kann länger sein als der Platz zwischen
    # Header und den unteren Buttons, daher wird hier geclippt/gescrollt ──
    viewport_top    = rows_y0
    viewport_bottom = HEIGHT - 80
    viewport_h      = max(row_h, viewport_bottom - viewport_top)
    content_h       = len(actions) * row_h
    max_scroll      = max(0, content_h - viewport_h)
    _ctrl_scroll    = max(0, min(_ctrl_scroll, max_scroll))

    # Card-Panel hinter der Tabelle (vorher: Zeilen schwebten direkt auf dem
    # Dim-Overlay, nur Divider-Linien dazwischen, kein umschliessendes Panel)
    table_panel = pygame.Rect(table_x - 16, viewport_top - 8, table_w + 36, viewport_h + 16)
    draw_card(base_surface, table_panel, radius=14, shadow=False)

    prev_clip = base_surface.get_clip()
    base_surface.set_clip(pygame.Rect(0, viewport_top, WIDTH, viewport_h))

    _ctrl_key_rects = []
    _ctrl_add_rects = []

    for i, action in enumerate(actions):
        ry = rows_y0 + i * row_h - _ctrl_scroll
        if ry + row_h < viewport_top or ry > viewport_bottom:
            continue  # außerhalb des sichtbaren Bereichs -> nicht zeichnen/klickbar
        is_listening = (_ctrl_listening == action)

        # Row highlight when listening
        if is_listening:
            hl = pygame.Surface((table_w, row_h - 2), pygame.SRCALPHA)
            hl.fill((200, 150, 0, 40))
            base_surface.blit(hl, (table_x, ry + 1))

        # Divider line between rows
        if i > 0:
            pygame.draw.line(base_surface, divider_c,
                             (table_x, ry), (table_x + table_w, ry), 1)

        row_cy = ry + row_h // 2

        # ── Action label (left side) ──
        lbl_color = (240, 200, 30) if is_listening else (255, 255, 255) if dark else (20, 20, 30)
        lbl = menu_font.render(get_action_label(action), True, lbl_color)
        base_surface.blit(lbl, (table_x + 14, row_cy - lbl.get_height() // 2))

        # ── Key buttons (right-aligned) ── Menü-Navigation: feste Standard-Tasten (nicht
        # entfernbar, siehe _MENU_NAV_DEFAULT_TUPLES) + eigene Extra-Tasten (entfernbar).
        if is_menu_nav_tab:
            default_keys = list(_MENU_NAV_DEFAULT_TUPLES.get(action, ()))
            keys_list = list(menu_nav_extras.get(action, []))
        else:
            default_keys = []
            keys_list = kb.get(action, [])
        btn_h = 36
        right_edge = table_x + table_w - 10

        # [+] button (rightmost)
        plus_w = 36
        plus_rect = pygame.Rect(right_edge - plus_w, row_cy - btn_h // 2, plus_w, btn_h)
        phov = plus_rect.collidepoint(mouse_pos) or nav_is_focused("controls_menu", i, len(actions) + 2)
        plus_bg  = (0, 160, 70) if phov else (0, 110, 50)
        pygame.draw.rect(base_surface, plus_bg, plus_rect, border_radius=6)
        ps = menu_font.render("+", True, (255, 255, 255))
        base_surface.blit(ps, (plus_rect.centerx - ps.get_width() // 2,
                               plus_rect.centery - ps.get_height() // 2))
        _ctrl_add_rects.append((action, plus_rect))
        cursor_x = right_edge - plus_w - 8

        if is_listening:
            # Pulsing "Taste drücken..." badge
            ws = menu_font.render(get_text("press_a_key"), True, (240, 200, 30))
            ww = ws.get_width() + 24
            wrect = pygame.Rect(cursor_x - ww, row_cy - btn_h // 2, ww, btn_h)
            pygame.draw.rect(base_surface, (70, 55, 10), wrect, border_radius=8)
            pygame.draw.rect(base_surface, (220, 175, 30), wrect, width=2, border_radius=8)
            base_surface.blit(ws, (wrect.centerx - ws.get_width() // 2,
                                   wrect.centery - ws.get_height() // 2))
        else:
            _ARROW_DIRS = {
                pygame.K_LEFT: "left", pygame.K_RIGHT: "right",
                pygame.K_UP: "up",    pygame.K_DOWN: "down",
            }
            for kcode in reversed(keys_list):
                arrow_dir = _ARROW_DIRS.get(kcode)
                kname     = key_display_name(kcode)
                is_conflict = _key_use_count.get(kcode, 0) > 1
                bw        = 54 if arrow_dir else max(54, menu_font.size(kname)[0] + 22)
                brect     = pygame.Rect(cursor_x - bw, row_cy - btn_h // 2, bw, btn_h)
                hov       = brect.collidepoint(mouse_pos)
                btn_bg     = (160, 40, 40)   if hov else (90, 65, 15)  if is_conflict else (55, 58, 80)  if dark else (195, 198, 215)
                btn_border = (220, 80, 80)   if hov else (235, 165, 40) if is_conflict else (100, 105, 135) if dark else (150, 155, 178)
                sym_color  = (255, 180, 180) if hov else (255, 210, 140) if is_conflict else (230, 230, 240) if dark else (20, 20, 30)
                pygame.draw.rect(base_surface, btn_bg,     brect, border_radius=7)
                pygame.draw.rect(base_surface, btn_border, brect, width=2, border_radius=7)
                if is_conflict and not hov:
                    warn = small_font.render("⚠", True, (255, 200, 60))
                    base_surface.blit(warn, (brect.right - warn.get_width() - 1, brect.top - warn.get_height() + 4))
                cx, cy2 = brect.centerx, brect.centery
                s = 9
                if arrow_dir == "left":
                    pts = [(cx+s, cy2-s), (cx-s, cy2), (cx+s, cy2+s)]
                elif arrow_dir == "right":
                    pts = [(cx-s, cy2-s), (cx+s, cy2), (cx-s, cy2+s)]
                elif arrow_dir == "up":
                    pts = [(cx-s, cy2+s), (cx, cy2-s), (cx+s, cy2+s)]
                elif arrow_dir == "down":
                    pts = [(cx-s, cy2-s), (cx, cy2+s), (cx+s, cy2-s)]
                else:
                    pts = None
                if pts:
                    pygame.draw.polygon(base_surface, sym_color, pts)
                else:
                    ks = menu_font.render(kname, True, sym_color)
                    base_surface.blit(ks, (brect.centerx - ks.get_width() // 2,
                                           brect.centery - ks.get_height() // 2))
                _ctrl_key_rects.append((action, kcode, brect))
                cursor_x -= bw + 6

            # Feste Standard-Tasten der Menü-Navigation - gedämpfte Optik ohne Hover/
            # Konflikt-Feedback, absichtlich NICHT in _ctrl_key_rects (Klick = kein Effekt).
            for kcode in reversed(default_keys):
                arrow_dir = _ARROW_DIRS.get(kcode)
                kname     = key_display_name(kcode)
                bw        = 54 if arrow_dir else max(54, menu_font.size(kname)[0] + 22)
                brect     = pygame.Rect(cursor_x - bw, row_cy - btn_h // 2, bw, btn_h)
                btn_bg, btn_border, sym_color = ((45, 47, 60), (90, 92, 110), (170, 170, 185)) \
                    if dark else ((225, 225, 235), (190, 190, 200), (90, 90, 100))
                pygame.draw.rect(base_surface, btn_bg,     brect, border_radius=7)
                pygame.draw.rect(base_surface, btn_border, brect, width=2, border_radius=7)
                cx, cy2 = brect.centerx, brect.centery
                s = 9
                if arrow_dir == "left":
                    pts = [(cx+s, cy2-s), (cx-s, cy2), (cx+s, cy2+s)]
                elif arrow_dir == "right":
                    pts = [(cx-s, cy2-s), (cx+s, cy2), (cx-s, cy2+s)]
                elif arrow_dir == "up":
                    pts = [(cx-s, cy2+s), (cx, cy2-s), (cx+s, cy2+s)]
                elif arrow_dir == "down":
                    pts = [(cx-s, cy2-s), (cx, cy2+s), (cx+s, cy2-s)]
                else:
                    pts = None
                if pts:
                    pygame.draw.polygon(base_surface, sym_color, pts)
                else:
                    ks = menu_font.render(kname, True, sym_color)
                    base_surface.blit(ks, (brect.centerx - ks.get_width() // 2,
                                           brect.centery - ks.get_height() // 2))
                cursor_x -= bw + 6

    base_surface.set_clip(prev_clip)

    # Scroll-Hinweis + einfache Scrollbar, nur wenn überhaupt gescrollt werden kann
    if max_scroll > 0:
        sb_track = pygame.Rect(table_x + table_w + 10, viewport_top, 6, viewport_h)
        pygame.draw.rect(base_surface, divider_c, sb_track, border_radius=3)
        sb_h = max(24, int(viewport_h * viewport_h / content_h))
        sb_y = viewport_top + int((viewport_h - sb_h) * (_ctrl_scroll / max_scroll))
        pygame.draw.rect(base_surface, (140, 145, 170) if dark else (110, 115, 140),
                         pygame.Rect(sb_track.x, sb_y, sb_track.w, sb_h), border_radius=3)
        scroll_hint = small_font.render(get_text("controls_scroll_hint"), True, (150, 150, 170) if dark else (110, 110, 130))
        base_surface.blit(scroll_hint, (WIDTH // 2 - scroll_hint.get_width() // 2, viewport_bottom + 4))

    # Bottom buttons
    btn_y = HEIGHT - 58
    reset_rect = pygame.Rect(WIDTH // 2 - 222, btn_y, 214, 44)
    back_rect  = pygame.Rect(WIDTH // 2 + 8,   btn_y, 214, 44)
    for bi, (rect, label, accent, use_small) in enumerate([
        (reset_rect, get_text("reset_defaults"), (180, 120, 0), True),
        (back_rect,  get_text("back"),             (70, 120, 200), False),
    ]):
        hov = rect.collidepoint(mouse_pos) or nav_is_focused("controls_menu", len(actions) + bi, len(actions) + 2)
        bg = (80, 60, 10) if (hov and rect is reset_rect) else \
             (40, 70, 140) if (hov and rect is back_rect) else \
             (50, 52, 68) if dark else (195, 198, 215)
        pygame.draw.rect(base_surface, bg, rect, border_radius=8)
        pygame.draw.rect(base_surface, accent, rect, width=2, border_radius=8)
        f = small_font if use_small else menu_font
        ls = f.render(label, True, (255, 255, 255) if dark or hov else (20, 20, 30))
        base_surface.blit(ls, (rect.centerx - ls.get_width() // 2,
                               rect.centery - ls.get_height() // 2))

    pygame.display.flip()
    return _ctrl_key_rects, _ctrl_add_rects, reset_rect, back_rect, _ctrl_tab_rects


def draw_settings_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    # Titel mit Schatten
    title = font.render(get_text('settings'), True, get_text_color())
    title_shadow = font.render(get_text('settings'), True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH//2 - title.get_width()//2 + 2, 62))
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 60))

    # Coin-Anzeige
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    
    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))

    # GUI Scale slider
    gui_scale = max(0.5, min(2.0, settings.get("gui_scale", 1.0)))
    slider_cx = WIDTH // 2
    slider_y = 125
    slider_w = 220
    track_rect = pygame.Rect(slider_cx - slider_w // 2, slider_y - 4, slider_w, 8)
    pygame.draw.rect(base_surface, get_panel_color(), track_rect, border_radius=4)
    pygame.draw.rect(base_surface, (150, 180, 220), track_rect, width=2, border_radius=4)
    t_frac = (gui_scale - 0.5) / 1.5
    handle_x = int(track_rect.x + t_frac * slider_w)
    # Markierung bei 100% (frac = 0.5/1.5 = 1/3)
    mark_x = int(track_rect.x + (1.0 / 3.0) * slider_w)
    pygame.draw.circle(base_surface, (100, 200, 120), (mark_x, slider_y), 4)
    rec_txt = small_font.render(get_text("recommended_label"), True, (100, 200, 120))
    base_surface.blit(rec_txt, (mark_x - rec_txt.get_width() // 2, slider_y + 13))
    pygame.draw.circle(base_surface, (80, 140, 220), (handle_x, slider_y), 10)
    pygame.draw.circle(base_surface, (200, 220, 255), (handle_x, slider_y), 10, 2)
    lbl = menu_font.render(get_text("gui_size_label"), True, get_text_color())
    base_surface.blit(lbl, (slider_cx - slider_w // 2 - lbl.get_width() - 10, slider_y - lbl.get_height() // 2))
    val_txt = menu_font.render(f"{int(gui_scale * 100)}%", True, get_text_color())
    base_surface.blit(val_txt, (slider_cx + slider_w // 2 + 10, slider_y - val_txt.get_height() // 2))

    # Hintergrund-Toggles (Sterne / Ringe / Animiert / Mono / Boden)
    _bg_toggle_defs = [
        ("bg_stars",          get_text("bg_stars_toggle")),
        ("bg_rings_enabled",  get_text("bg_rings_toggle")),
        ("bg_animated",       get_text("bg_animated_toggle")),
        ("mono_bg",           get_text("bg_mono_toggle")),
        ("show_floor_line",   get_text("bg_floor_toggle")),
    ]
    _toggle_total_w = 5 * 80 + 4 * 8
    _toggle_x0 = WIDTH // 2 - _toggle_total_w // 2
    _toggle_y = 158
    _stgs_nav_total = len(_bg_toggle_defs) + 6  # 5 Toggles + 6 Buttons (Slider bleibt Maus-only)
    bg_lbl = menu_font.render(get_text("background_label"), True, get_text_color())
    base_surface.blit(bg_lbl, (_toggle_x0 - bg_lbl.get_width() - 10, _toggle_y - bg_lbl.get_height() // 2 + 13))
    for ti, (skey, slabel) in enumerate(_bg_toggle_defs):
        default = False if skey == "mono_bg" else True
        active = settings.get(skey, default)
        tx = _toggle_x0 + ti * 88
        tr = pygame.Rect(tx, _toggle_y, 80, 26)
        bg_col = (70, 160, 90) if active else get_button_color()
        border_col = (100, 200, 120) if active else (160, 160, 160)
        if nav_is_focused("settings_menu", ti, _stgs_nav_total):
            border_col = (255, 255, 255)
        pygame.draw.rect(base_surface, bg_col, tr, border_radius=8)
        pygame.draw.rect(base_surface, border_col, tr, width=2, border_radius=8)
        ts = small_font.render(slabel, True, (255, 255, 255) if active else get_text_color())
        base_surface.blit(ts, (tr.centerx - ts.get_width() // 2, tr.centery - ts.get_height() // 2))

    # Buttons: Language, Dark Mode, Sound, Profiles, Steuerung, Mehr, Back
    start_x = WIDTH//2 - 180
    start_y = 193
    gap = 60
    dark_mode_text = get_text('light_mode') if is_dark_mode() else get_text('dark_mode')
    btn_texts = [get_text('language'), dark_mode_text, get_text('sound'), get_text('controls_menu_btn'), get_text('more'), get_text('back')]
    dark_mode_color = (0, 150, 0) if is_dark_mode() else (150, 100, 200)
    btn_colors = [(100, 200, 100), dark_mode_color, (100, 150, 200), (80, 180, 200), (200, 150, 80), (150, 150, 150)]

    for i, (text, color) in enumerate(zip(btn_texts, btn_colors)):
        rect = pygame.Rect(start_x, start_y + i*gap, 360, 48)
        is_hovered = rect.collidepoint(mouse_pos) or nav_is_focused("settings_menu", len(_bg_toggle_defs) + i, _stgs_nav_total)
        # Randfarbe = thematische Akzentfarbe je Button (Grün=Sprache, Blau=Sound, ...)
        draw_ui_button(base_surface, rect, text, font=menu_font, hovered=is_hovered,
                        border_color=color, hover_border_color=color, hover_border_width=3)


    # Export status textbox
    if last_export_message:
        export_bg = pygame.Rect(50, HEIGHT - 80, WIDTH - 100, 30)
        pygame.draw.rect(base_surface, get_panel_color(), export_bg, border_radius=8)
        pygame.draw.rect(base_surface, (180, 180, 180), export_bg, width=1, border_radius=8)
        export_text = small_font.render(last_export_message, True, get_text_color())
        base_surface.blit(export_text, (export_bg.x + 10, export_bg.y + 5))

    # Draw overlays
    cleanup_overlays()
    for ov in overlays:
        txt = small_font.render(ov["text"], True, ov["color"])
        base_surface.blit(txt, ((WIDTH - txt.get_width()) // 2, 100))
    draw_achievement_popups(base_surface)

    # Footer (theme-aware - vorher immer hell, egal ob Dark Mode aktiv war)
    footer_bg = pygame.Rect(0, HEIGHT - 30, WIDTH, 30)
    pygame.draw.rect(base_surface, get_panel_color(), footer_bg)
    pygame.draw.line(base_surface, (90, 90, 100) if is_dark_mode() else (200, 200, 200), (0, HEIGHT - 30), (WIDTH, HEIGHT - 30), 1)

    footer_text = small_font.render(get_text("back_to_main"), True, get_text_color())
    base_surface.blit(footer_text, (WIDTH // 2 - footer_text.get_width() // 2, HEIGHT - 25))

    pygame.display.flip()


def draw_settings_more_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    title = font.render("Mehr", True, get_text_color())
    title_shadow = font.render("Mehr", True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH//2 - title.get_width()//2 + 2, 62))
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 60))

    start_x = WIDTH//2 - 180
    start_y = 240
    gap = 70
    btn_texts  = ["Experimental", get_text('export_language'), get_text('back')]
    btn_colors = [(200, 150, 100), (150, 100, 200), (150, 150, 150)]

    rects = []
    for i, (text, color) in enumerate(zip(btn_texts, btn_colors)):
        rect = pygame.Rect(start_x, start_y + i * gap, 360, 48)
        rects.append(rect)
        is_hov = rect.collidepoint(mouse_pos) or nav_is_focused("settings_more_menu", i, len(btn_texts))
        draw_ui_button(base_surface, rect, text, font=menu_font, hovered=is_hov,
                        border_color=color, hover_border_color=(200, 220, 255), hover_border_width=3)

    pygame.display.flip()
    return rects


def draw_experimental_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    # Titel mit Schatten
    title = font.render("Experimental", True, get_text_color())
    title_shadow = font.render("Experimental", True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH//2 - title.get_width()//2 + 2, 62))
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 60))

    # Buttons
    start_x = WIDTH//2 - 180
    start_y = 180
    gap = 64
    btn_texts = [get_text('back')]

    for i, text in enumerate(btn_texts):
        rect = pygame.Rect(start_x, start_y + i*gap, 360, 48)
        is_hovered = rect.collidepoint(mouse_pos) or nav_is_focused("experimental_menu", i, len(btn_texts))
        draw_ui_button(base_surface, rect, text, font=menu_font, hovered=is_hovered)

    # Footer (theme-aware - vorher immer hell, egal ob Dark Mode aktiv war)
    footer_bg = pygame.Rect(0, HEIGHT - 30, WIDTH, 30)
    pygame.draw.rect(base_surface, get_panel_color(), footer_bg)
    pygame.draw.line(base_surface, (90, 90, 100) if is_dark_mode() else (200, 200, 200), (0, HEIGHT - 30), (WIDTH, HEIGHT - 30), 1)

    footer_text = small_font.render("ESC/M: Back to Settings", True, get_text_color())
    base_surface.blit(footer_text, (WIDTH // 2 - footer_text.get_width() // 2, HEIGHT - 25))

    pygame.display.flip()


def draw_settings_sound_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    # Titel mit Schatten
    title = font.render(get_text("sound"), True, get_text_color())
    title_shadow = font.render(get_text("sound"), True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH//2 - title.get_width()//2 + 2, 62))
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 60))

    # Buttons
    start_x = WIDTH//2 - 180
    start_y = 180
    gap = 64
    enabled_text = get_text("sound_enabled") + ": " + (get_text("on") if sound_enabled else get_text("off"))

    toggle_rect = pygame.Rect(start_x, start_y, 360, 48)
    back_rect = pygame.Rect(start_x, start_y + gap, 360, 48)
    _ssm_nav_total = 2 + 7 + 7 + len(INGAME_MUSIC_TRACKS)  # Toggle+Back + 7 Minus + 7 Plus + Tracks

    # Draw toggle
    is_hovered = toggle_rect.collidepoint(mouse_pos) or nav_is_focused("settings_sound_menu", 0, _ssm_nav_total)
    draw_ui_button(base_surface, toggle_rect, enabled_text, font=menu_font, hovered=is_hovered)

    # Draw back
    is_hovered = back_rect.collidepoint(mouse_pos) or nav_is_focused("settings_sound_menu", 1, _ssm_nav_total)
    draw_ui_button(base_surface, back_rect, get_text('back'), font=menu_font, hovered=is_hovered)

    # Volume controls
    volumes = [
        (get_text("menu_click_label"), "menu_click_volume"),
        (get_text("click_label"), "click_volume"),
        (get_text("click_back_label"), "click_back_volume"),
        (get_text("hit_label"), "hit_volume"),
        (get_text("gameover_label"), "gameover_volume"),
        (get_text("menu_music_label"), "menu_music_volume"),
        (get_text("ingame_music_label"), "ingame_music_volume"),
    ]
    minus_rects = []
    plus_rects = []
    track_rects = []
    y = start_y + 2 * gap
    for vi, (label, key) in enumerate(volumes):
        # Label
        label_surf = small_font.render(label + ":", True, get_text_color())
        base_surface.blit(label_surf, (start_x, y))
        # - button
        minus_rect = pygame.Rect(start_x + 150, y, 40, 30)
        is_hovered = minus_rect.collidepoint(mouse_pos) or nav_is_focused("settings_sound_menu", 2 + vi, _ssm_nav_total)
        color = get_button_hover_color() if is_hovered else get_button_color()
        pygame.draw.rect(base_surface, color, minus_rect, border_radius=8)
        minus_text = menu_font.render("-", True, get_text_color())
        base_surface.blit(minus_text, (minus_rect.centerx - minus_text.get_width()//2, minus_rect.centery - minus_text.get_height()//2))
        minus_rects.append((minus_rect, key, -0.1))
        # Value
        val = settings.get(key, 0.8)
        val_text = small_font.render(f"{val:.1f}", True, get_text_color())
        base_surface.blit(val_text, (start_x + 200, y + 5))
        # + button
        plus_rect = pygame.Rect(start_x + 250, y, 40, 30)
        is_hovered = plus_rect.collidepoint(mouse_pos) or nav_is_focused("settings_sound_menu", 9 + vi, _ssm_nav_total)
        color = get_button_hover_color() if is_hovered else get_button_color()
        pygame.draw.rect(base_surface, color, plus_rect, border_radius=8)
        plus_text = menu_font.render("+", True, get_text_color())
        base_surface.blit(plus_text, (plus_rect.centerx - plus_text.get_width()//2, plus_rect.centery - plus_text.get_height()//2))
        plus_rects.append((plus_rect, key, 0.1))

        # Ingame-Musik-Track-Auswahl: rechts neben der Lautstärke dieser Zeile.
        # Anklicken wählt den Track UND spielt ihn für max. 5s zum Vorhören ab;
        # ein anderer Track anklicken stoppt die laufende Vorschau sofort.
        if key == "ingame_music_volume":
            chosen_track = settings.get("ingame_music_track", "standard")
            track_btn_w, track_btn_h, track_gap = 72, 30, 6
            tx = start_x + 300
            for ti, (track_key, track_label, _filename) in enumerate(INGAME_MUSIC_TRACKS):
                tr = pygame.Rect(tx, y, track_btn_w, track_btn_h)
                is_selected = (track_key == chosen_track)
                previewing = is_previewing_track(track_key)
                is_hovered = tr.collidepoint(mouse_pos) or nav_is_focused("settings_sound_menu", 16 + ti, _ssm_nav_total)
                if previewing:
                    bg, border = (220, 160, 60), (255, 200, 100)
                elif is_selected:
                    bg, border = (70, 160, 90), (100, 200, 120)
                else:
                    bg = get_button_hover_color() if is_hovered else get_button_color()
                    border = (180, 180, 180)
                pygame.draw.rect(base_surface, bg, tr, border_radius=8)
                pygame.draw.rect(base_surface, border, tr, width=2, border_radius=8)
                txt_col = (255, 255, 255) if (is_selected or previewing) else get_text_color()
                txt = small_font.render(track_label, True, txt_col)
                base_surface.blit(txt, (tr.centerx - txt.get_width() // 2, tr.centery - txt.get_height() // 2))
                track_rects.append((tr, track_key))
                tx += track_btn_w + track_gap

        y += 40

    # Footer (theme-aware - vorher immer hell, egal ob Dark Mode aktiv war)
    footer_bg = pygame.Rect(0, HEIGHT - 30, WIDTH, 30)
    pygame.draw.rect(base_surface, get_panel_color(), footer_bg)
    pygame.draw.line(base_surface, (90, 90, 100) if is_dark_mode() else (200, 200, 200), (0, HEIGHT - 30), (WIDTH, HEIGHT - 30), 1)

    footer_text = small_font.render("ESC/M: Back to Settings", True, get_text_color())
    base_surface.blit(footer_text, (WIDTH // 2 - footer_text.get_width() // 2, HEIGHT - 25))

    pygame.display.flip()
    return toggle_rect, back_rect, minus_rects, plus_rects, track_rects


# Kleine Umleitungsfunktionen zur Wiederverwendung bestehender Abläufe
def open_stats_from_profile_hub():
    """Wechselt vom Profil-Hub zum Statistiken-Bildschirm. Spielt einen Menü-Klick-Sound ab."""
    global in_profile_hub, showing_stats
    in_profile_hub = False
    showing_stats = True
    play_sound(menu_click_sound)

def set_more_to_admin():
    global in_more_menu, in_admin_login, in_admin_panel
    in_more_menu = False
    in_admin_panel = True
    play_sound(menu_click_sound)

def open_classic_game():
    """Öffnet JumpNDodge Classic (eingebettete Spiel.py-Logik)."""
    global in_more_menu, in_classic_game
    play_sound(menu_click_sound)
    in_more_menu = False
    in_classic_game = True


def run_classic_game():
    """JumpNDodge Classic — die Original-Spiellogik aus Spiel.py, eingebettet
    ins JumpNDodge-Fenster (kein separater Prozess, kein eigenes Fenster mehr).

    NICHT ANFASSEN beim UI/Stil-Redesign: Dieser Screen ist auf explizite
    Nutzer-Vorgabe bewusst von der "Screens hübscher machen"-Überarbeitung
    ausgenommen. Keine Farb-/Font-/Card-/Glow-Änderungen hier, auch nicht
    "nur fürs Theme" oder "nur der Rahmen" - das gilt für den kompletten
    Original-Code-Block unten (siehe Markierung).

    Die eigentliche Spielmechanik (Physik, Gegner, Kollisionen, Zeichnen)
    ist 1:1 aus Spiel.py übernommen und unverändert. Angepasst wurden nur
    die für die Einbettung nötigen Teile:
      - eigene 800x600-Zeichenfläche statt eigenem Fenster
      - ESC/M beendet Classic und kehrt ins Mehr-Menü zurück, statt das
        ganze Programm zu schließen
      - oben mittig wird "ESC: Zurück" (übersetzt) eingeblendet
    """
    global in_classic_game, in_more_menu, base_surface

    # Musik stoppen, solange Classic läuft
    stop_music()

    # Eigene Oberfläche in der Original-Auflösung von Spiel.py (800x600)
    screen = pygame.Surface((800, 600))

    # ============================================================================
    # NICHT ANFASSEN - Original-Code aus Spiel.py, 1:1 unverändert (siehe Docstring
    # oben). Explizit AUSSERHALB des UI/Stil-Redesigns. Keine Farb-/Font-/Card-/
    # Glow-Edits in diesem Block, auch nicht kleine "nur fürs Theme"-Anpassungen.
    # ============================================================================
    WIDTH, HEIGHT = 800, 600

    WHITE = (255, 255, 255)
    GREEN = (0, 255, 0)
    BLUE = (0, 0, 255)
    RED = (255, 0, 0)

    player_size = 50
    player_color = BLUE
    player_x = WIDTH // 2
    player_y = HEIGHT - 70 - player_size  # Spieler startet auf der Plattform
    player_velocity = 5
    jump_height = 10
    is_jumping = False
    gravity = 0.5
    jump_speed = 10
    y_velocity = 0
    health = 3

    enemy_size = 50
    enemy_color = RED
    enemy_velocity = 2
    enemy_spawn_interval = 2000  # Millisekunden zwischen den Gegner-Spawns
    last_enemy_spawn_time = pygame.time.get_ticks()

    enemies = []

    platform_width = WIDTH
    platform_height = 20
    platform_x = 0
    platform_y = HEIGHT - platform_height - 50
    platform_color = GREEN

    score = 0

    classic_clock = pygame.time.Clock()
    running = True

    def spawn_enemy():
        side = random.choice(['left', 'right'])
        if side == 'left':
            x = -enemy_size
            direction = 1
        else:
            x = WIDTH
            direction = -1
        y = platform_y - enemy_size
        enemies.append({'x': x, 'y': y, 'direction': direction})

    while running:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            # Zusätzlich zum Original: ESC/M verlässt Classic (statt Programmende)
            if event.type == pygame.KEYDOWN and (event.key == pygame.K_ESCAPE or is_back_key(event.key)):
                running = False

        keys = pygame.key.get_pressed()
        if keys[pygame.K_a]:
            player_x -= player_velocity
        if keys[pygame.K_d]:
            player_x += player_velocity
        if keys[pygame.K_SPACE] and not is_jumping:
            is_jumping = True
            y_velocity = -jump_speed

        # Schwerkraft und Springen
        if is_jumping:
            player_y += y_velocity
            y_velocity += gravity
            if player_y >= platform_y - player_size:
                player_y = platform_y - player_size
                is_jumping = False
                y_velocity = 0

        # Gegner Spawnen
        current_time = pygame.time.get_ticks()
        if current_time - last_enemy_spawn_time > enemy_spawn_interval:
            spawn_enemy()
            last_enemy_spawn_time = current_time

        # Gegner Bewegung
        for enemy in enemies:
            enemy['x'] += enemy['direction'] * enemy_velocity

        # Kollisionserkennung
        player_rect = pygame.Rect(player_x, player_y, player_size, player_size)
        for enemy in enemies[:]:
            enemy_rect = pygame.Rect(enemy['x'], enemy['y'], enemy_size, enemy_size)
            if player_rect.colliderect(enemy_rect):
                if player_y + player_size - y_velocity <= enemy['y']:
                    enemies.remove(enemy)
                    score += 1  # Erhöhe den Score, wenn ein Gegner besiegt wird
                else:
                    health -= 1
                    player_x = WIDTH // 2
                    player_y = platform_y - player_size  # Spieler wird wieder auf der Plattform gespawnt
                    if health == 0:
                        print("Game Over")
                        running = False

        # Entferne Gegner, die den Bildschirm verlassen haben
        enemies = [enemy for enemy in enemies if 0 <= enemy['x'] <= WIDTH]

        # Bildschirm füllen
        screen.fill(WHITE)

        # Plattform zeichnen
        pygame.draw.rect(screen, platform_color, (platform_x, platform_y, platform_width, platform_height))

        # Spieler zeichnen
        pygame.draw.rect(screen, player_color, (player_x, player_y, player_size, player_size))

        # Gegner zeichnen
        for enemy in enemies:
            pygame.draw.rect(screen, enemy_color, (enemy['x'], enemy['y'], enemy_size, enemy_size))

        # Gesundheit und Score anzeigen
        classic_font = pygame.font.SysFont(None, 36)
        health_text = classic_font.render(f"Health: {health}", True, (0, 0, 0))
        score_text = classic_font.render(f"Score: {score}", True, (0, 0, 0))
        screen.blit(health_text, (10, 10))
        screen.blit(score_text, (10, 50))

        # Zusätzlich zum Original: "ESC: Zurück" oben mittig, in allen Sprachen übersetzt
        esc_label = small_font.render(get_text("esc_back"), True, (0, 0, 0))
        screen.blit(esc_label, (WIDTH // 2 - esc_label.get_width() // 2, 6))

        # ===================== Ende Original-Code aus Spiel.py =====================

        # In das (skalierbare) JumpNDodge-Fenster einblenden - mit Letterboxing:
        # Classic ist fest 800x600 (4:3), das Fenster kann aber jedes Seitenverhältnis
        # haben. Bisher wurde einfach auf die volle Fenstergröße gestreckt (verzerrt
        # das Bild bei jedem Nicht-4:3-Fenster). Jetzt: seitenverhältnistreu skalieren
        # und links/rechts bzw. oben/unten mit schwarzen Balken auffüllen.
        sync_window_size()
        base_w, base_h = base_surface.get_width(), base_surface.get_height()
        fit_scale = min(base_w / 800, base_h / 600)
        scaled_w, scaled_h = max(1, int(800 * fit_scale)), max(1, int(600 * fit_scale))
        scaled = pygame.transform.smoothscale(screen, (scaled_w, scaled_h))
        base_surface.fill((0, 0, 0))
        base_surface.blit(scaled, ((base_w - scaled_w) // 2, (base_h - scaled_h) // 2))
        pygame.display.flip()
        classic_clock.tick(30)

    in_classic_game = False
    in_more_menu = True
    # Menü-Musik wieder starten, sobald Classic verlassen wird
    if sound_enabled and menu_music_path:
        play_menu_music()

def back_to_main_from_more():
    play_sound(click_back_sound)
    global in_more_menu, in_menu
    in_more_menu = False
    in_menu = True

def open_experimental_menu():
    play_sound(menu_click_sound)
    global in_settings_menu, in_experimental_menu
    in_settings_menu = False
    in_experimental_menu = True

def open_settings_sound_menu():
    play_sound(menu_click_sound)
    global in_settings_menu, in_settings_sound_menu
    in_settings_menu = False
    in_settings_sound_menu = True

def open_controls_menu():
    play_sound(menu_click_sound)
    global in_settings_menu, in_controls_menu
    in_settings_menu = False
    in_controls_menu = True

def back_to_main_from_settings():
    play_sound(click_back_sound)
    global in_settings_menu, in_menu
    in_settings_menu = False
    in_menu = True

def back_to_settings_from_experimental():
    play_sound(click_back_sound)
    global in_experimental_menu, in_settings_more_menu
    in_experimental_menu = False
    in_settings_more_menu = True

def back_to_settings_from_sound():
    play_sound(click_back_sound)
    global in_settings_sound_menu, in_settings_menu
    in_settings_sound_menu = False
    in_settings_menu = True

def back_to_main_from_daily_rewards():
    """Kehrt vom Daily Rewards Menü zurück zum Hauptmenü.

    Schließt das Daily Rewards Menü und startet den Loading Screen für den Übergang.
    Spielt einen Zurück-Klick-Sound ab.
    """
    global in_daily_rewards, in_menu
    in_daily_rewards = False
    play_sound(click_back_sound, "click_back_volume")
    start_loading_screen(False)

def finish_loading_to_main():
    """Beendet den Loading Screen und geht zum Hauptmenü.

    Setzt die Loading-Flags zurück und aktiviert das Hauptmenü.
    """
    global in_loading, in_menu
    in_loading = False
    in_menu = True

def draw_loading_screen():
    """Zeichnet den Loading Screen"""
    global loading_progress, in_loading, in_daily_rewards, in_menu, loading_to_daily_rewards

    # Berechne Fortschritt
    current_time = pygame.time.get_ticks()
    elapsed = current_time - loading_start_time
    loading_progress = min(100, (elapsed / loading_duration) * 100)

    # Hintergrund
    fill_bg(rings=True)

    # Loading Text
    loading_text = font.render(get_text("loading"), True, get_text_color())
    base_surface.blit(loading_text, (WIDTH // 2 - loading_text.get_width() // 2, HEIGHT // 2 - 50))

    # Progress Bar
    bar_width = 400
    bar_height = 20
    bar_x = WIDTH // 2 - bar_width // 2
    bar_y = HEIGHT // 2 + 20

    # Hintergrund der Progress Bar
    pygame.draw.rect(base_surface, get_panel_color(), (bar_x, bar_y, bar_width, bar_height), border_radius=10)

    # Fortschritt
    progress_width = int((loading_progress / 100) * bar_width)
    pygame.draw.rect(base_surface, (0, 255, 0), (bar_x, bar_y, progress_width, bar_height), border_radius=10)

    # Prozent Text
    percent_text = small_font.render(f"{int(loading_progress)}%", True, get_text_color())
    base_surface.blit(percent_text, (WIDTH // 2 - percent_text.get_width() // 2, bar_y + 30))

    # Wenn fertig, gehe zum entsprechenden Menü
    if loading_progress >= 100:
        in_loading = False
        if loading_to_daily_rewards:
            in_daily_rewards = True
        else:
            in_menu = True

    pygame.display.flip()

def draw_daily_rewards_menu():
    """Zeichnet das Daily Rewards Menü mit detaillierter Anzeige"""
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    
    # Titel mit Schatten
    title = font.render(get_text("daily_rewards"), True, get_text_color())
    title_shadow = font.render(get_text("daily_rewards"), True, (100, 100, 120))
    base_surface.blit(title_shadow, (WIDTH//2 - title.get_width()//2 + 3, 73))
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 70))
    
    # Coin-Anzeige
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    
    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))
    
    # Hauptpanel
    main_panel = pygame.Rect(50, 120, WIDTH - 100, HEIGHT - 200)
    pygame.draw.rect(base_surface, get_panel_color(), main_panel, border_radius=16)
    pygame.draw.rect(base_surface, (60, 80, 140), main_panel, width=3, border_radius=16)
    
    y_offset = 150
    
    # Status
    can_claim, message = can_claim_daily_reward()
    if can_claim:
        status_text = menu_font.render(get_text('daily_reward_available'), True, (0, 150, 0))
        base_surface.blit(status_text, (70, y_offset))
        y_offset += 50
        
        # Claim Button
        claim_rect = pygame.Rect(WIDTH//2 - 100, y_offset, 200, 50)
        is_hovered = claim_rect.collidepoint(mouse_pos) or nav_is_focused("daily_rewards", 0, 2)
        draw_ui_button(base_surface, claim_rect, get_text("claim_reward"), font=menu_font, hovered=is_hovered,
                        base_color=(0, 180, 0), hover_color=(0, 210, 40), text_color=(255, 255, 255),
                        border_color=(0, 120, 0), hover_border_color=(150, 255, 170), hover_border_width=3)
        y_offset += 80
        
    elif "Internet" in message:
        status_text = menu_font.render(get_text('daily_reward_no_internet'), True, (200, 100, 100))
        base_surface.blit(status_text, (70, y_offset))
        y_offset += 50
    else:
        status_text = menu_font.render(get_text('daily_reward_claimed'), True, (150, 150, 150))
        base_surface.blit(status_text, (70, y_offset))
        y_offset += 50
        
        # Countdown bis zum nächsten Reset
        countdown_text = small_font.render(get_text("next_reset_in"), True, get_text_color())
        base_surface.blit(countdown_text, (70, y_offset))
        y_offset += 30
        
        time_until_reset = get_time_until_reset()
        time_text = menu_font.render(time_until_reset, True, (255, 100, 100))
        base_surface.blit(time_text, (70, y_offset))
        y_offset += 60
    
    # Letzte Belohnung Details
    last_stats = stats.get("last_daily_reward_stats", {})
    if last_stats:
        last_title = menu_font.render(get_text("last_reward") + ":", True, get_text_color())
        base_surface.blit(last_title, (70, y_offset))
        y_offset += 40
        
        # Coins
        coins_text = small_font.render(f"{last_stats.get('total', 0)} {get_text('coins')}", True, (255, 215, 0))
        base_surface.blit(coins_text, (90, y_offset))
        y_offset += 25
        
        # Rarity
        rarity = last_stats.get('rarity', 'Normal')
        rarity_colors = {"Normal": (130, 130, 140), "Rare": (80, 150, 255), "Epic": (180, 80, 255)}
        rarity_color = rarity_colors.get(rarity, (130, 130, 140))
        rarity_label = get_text("rarity_label")
        rarity_name  = get_text(f"rarity_{rarity.lower()}")
        rarity_text  = small_font.render(f"{rarity_label}: {rarity_name}", True, rarity_color)
        base_surface.blit(rarity_text, (90, y_offset))
        y_offset += 25

        extra = last_stats.get('extra', 0)
        if extra > 0:
            extra_col = (180, 80, 255) if rarity == "Epic" else (80, 150, 255)
            extra_text = small_font.render(f"+{extra} {get_text('coins')} Extra", True, extra_col)
            base_surface.blit(extra_text, (90, y_offset))
            y_offset += 25
    
    # Zurück Button
    back_rect = pygame.Rect(WIDTH - 180, HEIGHT - 60, 150, 40)
    _dr_back_idx = 1 if can_claim else 0
    is_hovered = back_rect.collidepoint(mouse_pos) or nav_is_focused("daily_rewards", _dr_back_idx, _dr_back_idx + 1)
    draw_ui_button(base_surface, back_rect, get_text("back"), font=small_font, hovered=is_hovered, radius=10)

    pygame.display.flip()

# Help menu open
def open_help_menu():
    global in_more_menu, in_help, help_scroll, help_lines, help_lang
    in_more_menu = False
    in_help = True
    help_scroll = 0
    help_lang = languages[language_index]
    txt = HELP_TEXTS.get(help_lang, HELP_TEXTS.get('English', ''))
    # Pre-split into lines for rendering
    help_lines = txt.split('\n')
    play_sound(menu_click_sound)

# Draw help menu (scrollable)
def draw_help_menu():
    global help_scroll
    fill_bg(rings=True)
    title = font.render(get_text('help'), True, get_text_color())
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 20))

    # Card-Panel hinter dem scrollbaren Text (vorher: nackter Text direkt auf dem
    # Hintergrund, ohne Clipping - konnte beim Scrollen über Titel/Hint hinausragen)
    panel = pygame.Rect(20, 90, WIDTH - 40, HEIGHT - 146)
    draw_card(base_surface, panel, radius=14, shadow=False)

    prev_clip = base_surface.get_clip()
    base_surface.set_clip(panel.inflate(-16, -16))
    y0 = panel.y + 14 - help_scroll
    for line in help_lines:
        # Wrap if too long? We'll render raw lines; long lines may be clipped
        txt = small_font.render(line, True, get_text_color())
        base_surface.blit(txt, (panel.x + 20, y0))
        y0 += 26
    base_surface.set_clip(prev_clip)

    hint = small_font.render(get_text("scroll_back_hint"), True, get_text_color())
    base_surface.blit(hint, (WIDTH//2 - hint.get_width()//2, HEIGHT - 40))
    pygame.display.flip()
# =============================
# Statistics Screen (Statistiken-Bildschirm)
# =============================

# Liste der verfügbaren Statistiken-Kategorien als Tab-Namen
# Diese werden als Schlüssel für die Lokalisierung verwendet (z.B. "stat_highscores")
stat_categories = ["stat_highscores", "stat_achievements", "stat_total", "stat_modded"]

# Index des aktuell ausgewählten Tabs (0=Highscores, 1=Achievements, 2=Gesamtstatistiken, 3=Modded)
# Wird durch Tab-Wechsel-Funktionen modifiziert
current_stat_tab = 0

# Sub-tab index inside the Modded tab (0=Highscores, 1=Achievements, 2=Total)
modded_sub_tab = 0

# Liste von Rechtecken für die Tab-Buttons, jedes mit (rect, index)
# Wird in draw_statistics() gefüllt und für Mausinteraktionen verwendet
stat_tab_rects = []

# Vertikaler Scroll-Offset für das Scrollen im Statistiken-Bildschirm
# Erhöht sich beim Nach-unten-Scrollen, verringert sich beim Nach-oben-Scrollen
# Wird durch adjust_stat_scroll() modifiziert und in draw_statistics() verwendet
stat_scroll = 0


def switch_stat_tab():
    """Wechselt zum nächsten Statistiken-Tab in zyklischer Reihenfolge.

    Durchläuft alle verfügbaren Tabs (Highscores → Achievements → Gesamt → Highscores...)
    und aktualisiert die globale Variable current_stat_tab.

    Diese Funktion wird durch TAB-Taste oder Tab-Klicks aufgerufen.
    """
    global current_stat_tab
    _vis = [c for c in stat_categories
            if c != "stat_modded" or os.path.isfile(_MODDED_DATA_FILE)]
    current_stat_tab = (current_stat_tab + 1) % len(_vis)

def adjust_stat_scroll(delta):
    """Passt den vertikalen Scroll-Offset für den Statistiken-Bildschirm an.

    Verhindert negatives Scrolling und aktualisiert den Scroll-Offset basierend
    auf Mauswheel-Eingaben oder anderen Scroll-Events.

    Args:
        delta (int): Änderung des Scroll-Offsets in Pixeln
                     (positiv = nach unten scrollen, negativ = nach oben scrollen)
    """
    global stat_scroll
    stat_scroll = max(0, stat_scroll + delta)

def adjust_loadout_scroll(delta):
    """Passt den vertikalen Scroll-Offset für den Loadout-Bildschirm an.

    Ähnlich zu adjust_stat_scroll, aber für den Loadout-Bereich.
    Verhindert negatives Scrolling.

    Args:
        delta (int): Änderung des Scroll-Offsets in Pixeln
                     (positiv = nach unten scrollen, negativ = nach oben scrollen)
    """
    global loadout_scroll
    loadout_scroll = max(0, loadout_scroll + delta)


def draw_statistics():
    """Zeichnet den vollständigen Statistiken-Bildschirm mit Tabs und Scrolling.

    Diese Funktion rendert das komplette Statistics-Menü, einschließlich:
    - Hintergrund und dekorative Elemente
    - Tab-Buttons für Highscores, Achievements und Gesamtstatistiken
    - Content-Panel mit Scroll-barem Inhalt
    - Fading-Effekte für weiche Übergänge am Rand
    - Abhängig vom aktuellen Tab verschiedene Inhalte

    Behandelt auch Fehlerbehandlung und Fallback zum Hauptmenü bei Problemen.

    Global modifiziert:
        stat_tab_rects: Wird mit den Tab-Button-Rechtecken gefüllt
        showing_stats: Kann bei Fehlern zurückgesetzt werden
        in_menu: Kann bei Fehlern aktiviert werden
    """
    global stat_tab_rects, showing_stats, in_profile_hub, modded_sub_tab, current_stat_tab
    try:
        # Hintergrund mit Theme-abhängiger Farbe füllen
        fill_bg(rings=True)
        # Solide Panels über Kopf- und Fußbereich, damit Ringe Titel/Tabs/Zurück nicht überdecken
        pygame.draw.rect(base_surface, get_bg_color(), pygame.Rect(0, 0, WIDTH, 108))
        pygame.draw.rect(base_surface, get_bg_color(), pygame.Rect(0, HEIGHT - 65, WIDTH, 65))

        # Haupttitel "Statistiken" mit Schatten-Effekt für bessere Lesbarkeit
        title = font.render(get_text("stats"), True, get_text_color())
        title_shadow = font.render(get_text("stats"), True, (100, 100, 120))
        base_surface.blit(title_shadow, (WIDTH // 2 - title.get_width() // 2 + 3, 15))
        base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 12))

        # Modded-Tab nur anzeigen wenn modded_data.json existiert
        _visible_cats = [c for c in stat_categories
                         if c != "stat_modded" or os.path.isfile(_MODDED_DATA_FILE)]
        # current_stat_tab darf nicht auf einen unsichtbaren Tab zeigen
        if current_stat_tab >= len(_visible_cats):
            current_stat_tab = 0

        _stat_is_modded_active = (current_stat_tab == _visible_cats.index("stat_modded")) if "stat_modded" in _visible_cats else False
        _stat_nav_total = len(_visible_cats) + (3 if _stat_is_modded_active else 0)  # + 3 Sub-Tabs falls Modded aktiv

        # Erstelle und zeichne Tab-Buttons, zentriert im Fenster
        stat_tab_rects = []
        _tab_w   = 170
        _tab_gap = 8
        _tab_total = len(_visible_cats) * _tab_w + (len(_visible_cats) - 1) * _tab_gap
        _tab_x0  = (WIDTH - _tab_total) // 2
        for i, key in enumerate(_visible_cats):
            color = get_button_color() if i == current_stat_tab else get_panel_color()
            rect = pygame.Rect(_tab_x0 + i * (_tab_w + _tab_gap), 65, _tab_w, 35)
            pygame.draw.rect(base_surface, color, rect, border_radius=10)
            # Modded-Tab: goldener Rahmen
            brd = (220, 160, 0) if key == "stat_modded" and i == current_stat_tab else (180, 180, 180)
            if nav_is_focused("statistics", i, _stat_nav_total):
                brd = (255, 255, 255)
            pygame.draw.rect(base_surface, brd, rect, width=2, border_radius=10)
            label_surface = small_font.render(get_text(key), True, get_text_color())
            base_surface.blit(label_surface, (rect.centerx - label_surface.get_width() // 2,
                                               rect.centery - label_surface.get_height() // 2))
            stat_tab_rects.append((rect, i))

        # Haupt-Content-Panel für den scrollbaren Inhalt
        # Rundes Rechteck mit Rahmen für bessere UI-Abgrenzung
        content_panel = pygame.Rect(50, 160, WIDTH - 100, HEIGHT - 240)
        pygame.draw.rect(base_surface, get_panel_color(), content_panel, border_radius=16)
        pygame.draw.rect(base_surface, (60, 80, 140), content_panel, width=3, border_radius=16)

        # Parameter für Fading-Effekte am oberen und unteren Rand des Content-Panels
        # Elemente blenden weich aus, wenn sie den sichtbaren Bereich verlassen
        fade_top = 160  # Obere Grenze des Fade-Bereichs
        fade_bottom = HEIGHT - 80  # Untere Grenze des Fade-Bereichs
        fade_dist = 20  # Distanz über die das Fading stattfindet

        # Startposition für den scrollbaren Inhalt, angepasst durch stat_scroll
        y = 180 - stat_scroll

        # Tab 0: Highscores - Zeigt Rekorde pro Schwierigkeitsgrad in einer Tabelle
        if current_stat_tab == 0:
            # Tabellen-Header mit fester Position (nicht scrollend)
            # Hintergrund für die Header-Zeile
            header_bg = pygame.Rect(70, y, WIDTH - 140, 35)
            pygame.draw.rect(base_surface, get_panel_color(), header_bg, border_radius=8)
            pygame.draw.rect(base_surface, (60, 80, 140), header_bg, width=2, border_radius=8)

            # Header-Texte für die drei Spalten: Schwierigkeit, Rekord, Sitzung
            header_font = small_font
            diff_header = header_font.render(get_text("table_difficulty"), True, get_text_color())
            record_header = header_font.render(get_text("table_highscore"), True, get_text_color())
            session_header = header_font.render(get_text("table_session"), True, get_text_color())
            avg_header = header_font.render(get_text("avg_score"), True, get_text_color())

            # Header-Texte immer vollständig sichtbar positionieren
            base_surface.blit(diff_header, (90, y + 8))      # Schwierigkeit links
            base_surface.blit(record_header, (300, y + 8))   # Rekord mitte
            base_surface.blit(session_header, (450, y + 8))  # Sitzung rechts
            base_surface.blit(avg_header, (620, y + 8))      # Ø Score pro Schwierigkeit
            y += 45  # Nach Header Platz für Tabellenzeilen lassen

            # Tabellenzeilen für jede Schwierigkeitsstufe generieren
            _diff_col_map = {
                "Leicht": (0,200,0), "Mittel": (200,200,0), "Schwer": (200,100,0),
                "Hardcore": (200,0,0), "Unmöglich": (150,0,150), "Albtraum": (139,0,0),
                "Hölle": (20,20,20), "Dynamisch": (0,150,200),
            }
            _dim = (120, 120, 140) if is_dark_mode() else (110, 110, 130)
            for i, diff in enumerate(difficulties):
                record      = saved_scores.get(diff, 0)
                session_val = session_scores.get(diff, 0)
                rec_time    = best_times.get(diff, 0)
                ses_time    = session_times.get(diff, 0)
                _diff_rounds = stats.get("rounds_by_diff", {}).get(diff, 0)
                avg_val = round(stats.get("score_sum_by_diff", {}).get(diff, 0) / _diff_rounds, 1) if _diff_rounds > 0 else 0

                alpha = 255
                if y < fade_top:
                    alpha = max(0, int(255 * (y - (fade_top - fade_dist)) / fade_dist))
                elif y + 36 > fade_bottom:
                    alpha = max(0, int(255 * ((fade_bottom - y) / fade_dist)))

                if alpha > 0:
                    row_bg = pygame.Rect(70, y, WIDTH - 140, 36)
                    pygame.draw.rect(base_surface, get_bg_color() if i % 2 == 0 else get_panel_color(), row_bg, border_radius=6)
                    pygame.draw.rect(base_surface, (180, 180, 180), row_bg, width=1, border_radius=6)

                    diff_name = get_difficulty_name(diff)
                    diff_color = _diff_col_map.get(diff, get_text_color())
                    for surf, x, dy in [
                        (small_font.render(diff_name, True, diff_color), 90, 10),
                    ]:
                        surf.set_alpha(alpha)
                        base_surface.blit(surf, (x, y + dy))
                    for score_val, time_val, col_x in [
                        (record,      rec_time, 320),
                        (session_val, ses_time, 470),
                    ]:
                        s_surf = small_font.render(str(score_val), True, get_text_color())
                        t_surf = small_font.render(f" {time_val}s", True, _dim)
                        s_surf.set_alpha(alpha)
                        t_surf.set_alpha(alpha)
                        base_surface.blit(s_surf, (col_x, y + 10))
                        base_surface.blit(t_surf, (col_x + s_surf.get_width(), y + 10))

                    avg_surf = small_font.render(str(avg_val), True, get_text_color())
                    avg_surf.set_alpha(alpha)
                    base_surface.blit(avg_surf, (620, y + 10))

                y += 40
        # Tab 1: Achievements – currently disabled
        elif current_stat_tab == 1:
            cx = WIDTH // 2
            cy = content_panel.centery
            # Draw padlock programmatically (no emoji needed)
            _lc = (120, 120, 140)
            _lx, _ly = cx - 18, cy - 72
            pygame.draw.rect(base_surface, _lc, pygame.Rect(_lx, _ly + 18, 36, 28), border_radius=5)
            pygame.draw.arc(base_surface, _lc, pygame.Rect(_lx + 6, _ly, 24, 26), 0, pi, 4)
            pygame.draw.circle(base_surface, (80, 80, 100), (cx, _ly + 31), 5)
            dis1 = menu_font.render(get_text("achievements_disabled_notice"), True, get_text_color())
            dis2 = small_font.render(get_text("future_version_notice"), True, (140, 140, 160))
            base_surface.blit(dis1, (cx - dis1.get_width() // 2, cy - 10))
            base_surface.blit(dis2, (cx - dis2.get_width() // 2, cy + 26))
        # (old achievements code kept below for future re-enable)
        elif False and current_stat_tab == -1:
            import math as _math
            ACHIEVEMENT_CATS = [
                ("🎯 Score",     ["score_10", "score_50", "score_100", "score_200", "score_500", "score_1000"]),
                ("🔁 Runden",    ["first_round", "five_rounds", "ten_rounds", "twenty_rounds", "fifty_rounds"]),
                ("🔥 Hardcore",  ["hardcore_start", "hardcore_survivor"]),
                ("⚡ Unmöglich", ["impossible_start", "impossible_survivor"]),
                ("💚 Pickups",   ["pickup_1up", "pickup_3x_1up", "pickup_10x_1up"]),
                ("💰 Coins",     ["coins_100", "coins_500", "coins_1000"]),
                ("🏆 Bonus",     ["no_damage_run", "speed_demon"]),
            ]
            has_any = any(achievements.get(c, False)
                          for _, codes in ACHIEVEMENT_CATS for c in codes)

            for cat_label, codes in ACHIEVEMENT_CATS:
                # Category header
                alpha_h = 255
                if y < fade_top:
                    alpha_h = max(0, int(255 * (y - (fade_top - fade_dist)) / fade_dist))
                elif y + 24 > fade_bottom:
                    alpha_h = max(0, int(255 * ((fade_bottom - y) / fade_dist)))
                if alpha_h > 0:
                    unlocked_count = sum(1 for c in codes if achievements.get(c, False))
                    all_done = unlocked_count == len(codes)
                    hdr_color = (255, 210, 50) if all_done else (160, 160, 210)
                    hdr_s = small_font.render(f"{cat_label}  {unlocked_count}/{len(codes)}", True, hdr_color)
                    hdr_s.set_alpha(alpha_h)
                    base_surface.blit(hdr_s, (75, y + 3))
                y += 26

                for code in codes:
                    if code not in achievement_definitions:
                        continue
                    unlocked = achievements.get(code, False)
                    title_txt = get_achievement_name(code)

                    alpha = 255
                    if y < fade_top:
                        alpha = max(0, int(255 * (y - (fade_top - fade_dist)) / fade_dist))
                    elif y + 30 > fade_bottom:
                        alpha = max(0, int(255 * ((fade_bottom - y) / fade_dist)))

                    if alpha > 0:
                        if unlocked:
                            bg_col    = (28, 78, 48)
                            border_col = (70, 190, 110)
                            text_col  = (190, 255, 210)
                            prefix    = "✔ "
                        else:
                            bg_col    = (38, 38, 52)
                            border_col = (65, 65, 88)
                            text_col  = (95, 95, 125)
                            prefix    = "🔒 "

                        ach_rect = pygame.Rect(75, y, WIDTH - 150, 30)
                        pygame.draw.rect(base_surface, bg_col, ach_rect, border_radius=7)
                        pygame.draw.rect(base_surface, border_col, ach_rect, width=2, border_radius=7)

                        if unlocked:
                            gv = int(160 + 60 * _math.sin(time.time() * 3 + codes.index(code) * 0.8))
                            pygame.draw.rect(base_surface, (gv, int(gv * 0.85), int(gv * 0.3)),
                                             ach_rect, width=1, border_radius=7)

                        row_txt = small_font.render(f"{prefix}{title_txt}", True, text_col)
                        row_txt.set_alpha(alpha)
                        base_surface.blit(row_txt, (83, y + 6))
                    y += 34

                y += 8  # Abstand zwischen Kategorien

            if not has_any:
                msg = small_font.render(get_text("no_achievements"), True, get_text_color())
                base_surface.blit(msg, (70, y))
        # Tab 2: Gesamtstatistiken - Zeigt aggregierte Spielstatistiken
        elif current_stat_tab == 2:
            # Immer normale (unmodded) Stats anzeigen — Modded-Stats nur im Modded-Tab
            total_time_seconds = int(stats.get("total_time_played", 0))
            total_time = str(timedelta(seconds=total_time_seconds))  # In HH:MM:SS formatieren
            rounds = stats.get("total_rounds", 0)
            oneups = stats.get("total_1ups", 0)
            coins  = stats.get("coins", 0)
            jumps  = stats.get("total_jumps", 0)
            deaths = stats.get("total_deaths", 0)
            coins_earned = stats.get("total_coins_earned", 0)

            # Liste der anzuzeigenden Statistik-Items mit Labels und Werten
            stats_items = [
                (get_text("total_time"), total_time),      # Lokalisierte Labels verwenden
                (get_text("rounds_played"), rounds),
                (get_text("1ups_collected"), oneups),
                (get_text("coins"), coins),
                (get_text("total_jumps"), jumps),
                (get_text("total_deaths"), deaths),
                (get_text("total_coins_earned"), coins_earned),
            ]

            # Jedes Statistik-Item als separate Zeile zeichnen
            for label, value in stats_items:
                # Fading-Alpha berechnen (wie bei anderen Tabs)
                alpha = 255
                if y < fade_top:
                    alpha = max(0, int(255 * (y - (fade_top - fade_dist)) / fade_dist))
                elif y + 40 > fade_bottom:
                    alpha = max(0, int(255 * ((fade_bottom - y) / fade_dist)))

                if alpha > 0:
                    # Statistik-Rechteck mit Rahmen
                    stat_rect = pygame.Rect(70, y, WIDTH - 140, 40)
                    pygame.draw.rect(base_surface, get_panel_color(), stat_rect, border_radius=8)
                    pygame.draw.rect(base_surface, (200, 200, 200), stat_rect, width=1, border_radius=8)
                    # Label links, Wert rechts
                    label_surf = small_font.render(f"{label}:", True, get_text_color())
                    value_surf = small_font.render(str(value), True, get_text_color())
                    label_surf.set_alpha(alpha)
                    value_surf.set_alpha(alpha)
                    base_surface.blit(label_surf, (80, y + 10))
                    base_surface.blit(value_surf, (300, y + 10))
                y += 45  # Zeilenhöhe für Statistiken

        # Tab 3: Modded – Sub-tabs + Mod-Info (nur sichtbar wenn modded_data.json existiert)
        elif current_stat_tab == _visible_cats.index("stat_modded") if "stat_modded" in _visible_cats else False:
            global modded_sub_tab
            dark = is_dark_mode()

            # ---- Sub-tab bar ----
            _sub_labels = ["Highscores", "Achievements", "Stats"]
            _sw, _sg = 140, 6
            _sx0 = (WIDTH - (len(_sub_labels) * _sw + (len(_sub_labels) - 1) * _sg)) // 2
            _sub_rects = []
            for si, sl in enumerate(_sub_labels):
                sr = pygame.Rect(_sx0 + si * (_sw + _sg), y, _sw, 32)
                sc = get_button_color() if si == modded_sub_tab else get_panel_color()
                pygame.draw.rect(base_surface, sc, sr, border_radius=8)
                _sub_brd = (255, 255, 255) if nav_is_focused("statistics", len(_visible_cats) + si, _stat_nav_total) else (220, 160, 0)
                pygame.draw.rect(base_surface, _sub_brd, sr, width=2, border_radius=8)
                ss = small_font.render(sl, True, get_text_color())
                base_surface.blit(ss, (sr.centerx - ss.get_width() // 2,
                                       sr.centery - ss.get_height() // 2))
                _sub_rects.append((sr, si))
                stat_tab_rects.append((sr, (3, si)))  # (main_tab, sub_tab) tuple for click handling
            y += 44

            # ---- Mod overview strip ----
            n_total  = len(mod_system.mods)
            n_active = mod_system.get_active_count()
            n_broken = sum(1 for m in mod_system.mods if m.error is not None)
            _ov_bg   = (40, 30, 10) if dark else (255, 248, 220)
            _ov_brd  = (200, 140, 0)
            _ov_rect = pygame.Rect(55, y, WIDTH - 110, 34)
            pygame.draw.rect(base_surface, _ov_bg,  _ov_rect, border_radius=8)
            pygame.draw.rect(base_surface, _ov_brd, _ov_rect, width=2, border_radius=8)
            _ov_text = f"Mods geladen: {n_total}   Aktiv: {n_active}   Fehler: {n_broken}"
            _ov_surf = small_font.render(_ov_text, True, (220, 160, 0))
            base_surface.blit(_ov_surf, (_ov_rect.centerx - _ov_surf.get_width() // 2,
                                          _ov_rect.centery - _ov_surf.get_height() // 2))
            y += 42

            # ---- Sub-tab content ----
            # Reuse the same rendering as the main tabs
            if modded_sub_tab == 0:
                # -- Modded Highscores --
                header_bg = pygame.Rect(70, y, WIDTH - 140, 35)
                pygame.draw.rect(base_surface, get_panel_color(), header_bg, border_radius=8)
                pygame.draw.rect(base_surface, (220, 160, 0), header_bg, width=2, border_radius=8)
                diff_header   = small_font.render(get_text("table_difficulty"), True, get_text_color())
                record_header = small_font.render(get_text("table_highscore"),  True, get_text_color())
                session_header= small_font.render(get_text("table_session"),    True, get_text_color())
                base_surface.blit(diff_header,    (90,  y + 8))
                base_surface.blit(record_header,  (300, y + 8))
                base_surface.blit(session_header, (450, y + 8))
                y += 45
                diff_colors = {"Leicht":(0,200,0),"Mittel":(200,200,0),"Schwer":(200,100,0),
                               "Hardcore":(200,0,0),"Unmöglich":(150,0,150),"Albtraum":(139,0,0),"Hölle":(20,20,20),"Dynamisch":(0,150,200)}
                _dim = (120,120,140) if is_dark_mode() else (110,110,130)
                for i, diff in enumerate(difficulties):
                    record      = modded_scores.get(diff, 0)
                    session_val = modded_session_scores.get(diff, 0)
                    rec_time    = modded_best_times.get(diff, 0)
                    ses_time    = modded_session_times.get(diff, 0)
                    alpha = 255
                    if y < fade_top:   alpha = max(0, int(255*(y-(fade_top-fade_dist))/fade_dist))
                    elif y+36>fade_bottom: alpha = max(0, int(255*((fade_bottom-y)/fade_dist)))
                    if alpha > 0:
                        row_bg = pygame.Rect(70, y, WIDTH-140, 36)
                        pygame.draw.rect(base_surface, get_bg_color() if i%2==0 else get_panel_color(), row_bg, border_radius=6)
                        pygame.draw.rect(base_surface, (180,180,180), row_bg, width=1, border_radius=6)
                        for surf, x, dy in [
                            (small_font.render(get_difficulty_name(diff), True, diff_colors.get(diff, get_text_color())), 90, 10),
                        ]:
                            surf.set_alpha(alpha)
                            base_surface.blit(surf, (x, y+dy))
                        for score_val, time_val, col_x in [
                            (record,      rec_time, 320),
                            (session_val, ses_time, 470),
                        ]:
                            s_surf = small_font.render(str(score_val), True, get_text_color())
                            t_surf = small_font.render(f" {time_val}s", True, _dim)
                            s_surf.set_alpha(alpha)
                            t_surf.set_alpha(alpha)
                            base_surface.blit(s_surf, (col_x, y+10))
                            base_surface.blit(t_surf, (col_x + s_surf.get_width(), y+10))
                    y += 40

            elif modded_sub_tab == 1:
                # -- Achievements (same as main tab 1) --
                cx = WIDTH // 2
                cy = content_panel.centery + 30
                _lc = (120, 120, 140)
                _lx, _ly = cx - 18, cy - 52
                pygame.draw.rect(base_surface, _lc, pygame.Rect(_lx, _ly + 18, 36, 28), border_radius=5)
                pygame.draw.arc(base_surface, _lc, pygame.Rect(_lx+6, _ly, 24, 26), 0, pi, 4)
                pygame.draw.circle(base_surface, (80,80,100), (cx, _ly+31), 5)
                dis1 = menu_font.render(get_text("achievements_disabled_notice"), True, get_text_color())
                dis2 = small_font.render(get_text("future_version_notice"), True, (140,140,160))
                base_surface.blit(dis1, (cx - dis1.get_width()//2, cy - 10))
                base_surface.blit(dis2, (cx - dis2.get_width()//2, cy + 26))

            elif modded_sub_tab == 2:
                # -- Modded Total Stats --
                total_time_seconds = int(modded_stats.get("total_time_played", 0))
                total_time = str(timedelta(seconds=total_time_seconds))
                stats_items = [
                    (get_text("total_time"),    total_time),
                    (get_text("rounds_played"), modded_stats.get("total_rounds", 0)),
                    (get_text("1ups_collected"),modded_stats.get("total_1ups", 0)),
                    (get_text("coins"),         modded_stats.get("coins", 0)),
                ]
                for label, value in stats_items:
                    alpha = 255
                    if y < fade_top:   alpha = max(0, int(255*(y-(fade_top-fade_dist))/fade_dist))
                    elif y+40>fade_bottom: alpha = max(0, int(255*((fade_bottom-y)/fade_dist)))
                    if alpha > 0:
                        sr = pygame.Rect(70, y, WIDTH-140, 40)
                        pygame.draw.rect(base_surface, get_panel_color(), sr, border_radius=8)
                        pygame.draw.rect(base_surface, (200,200,200), sr, width=1, border_radius=8)
                        ls = small_font.render(f"{label}:", True, get_text_color())
                        vs = small_font.render(str(value),  True, get_text_color())
                        ls.set_alpha(alpha); vs.set_alpha(alpha)
                        base_surface.blit(ls, (80,  y+10))
                        base_surface.blit(vs, (300, y+10))
                    y += 45

            # ---- Mod list (always shown below sub-tab content) ----
            y += 10
            if mod_system.mods:
                for mod in mod_system.mods:
                    alpha = 255
                    if y < fade_top:   alpha = max(0, int(255*(y-(fade_top-fade_dist))/fade_dist))
                    elif y+32>fade_bottom: alpha = max(0, int(255*((fade_bottom-y)/fade_dist)))
                    if alpha <= 0:
                        y += 36
                        continue
                    is_broken = mod.error is not None
                    mr = pygame.Rect(60, y, WIDTH-120, 32)
                    if is_broken:
                        mbg = (60, 20, 20) if dark else (255, 220, 220)
                        mbd = (200, 60, 60)
                    elif mod.active:
                        mbg = (20, 50, 30) if dark else (220, 255, 230)
                        mbd = (60, 180, 80)
                    else:
                        mbg = (40, 40, 55) if dark else (230, 230, 240)
                        mbd = (130, 130, 150)
                    pygame.draw.rect(base_surface, mbg, mr, border_radius=7)
                    pygame.draw.rect(base_surface, mbd, mr, width=2, border_radius=7)
                    status_str = "BROKEN" if is_broken else ("ON" if mod.active else "OFF")
                    status_col = (220,60,60) if is_broken else ((60,200,80) if mod.active else (150,150,150))
                    name_s   = small_font.render(mod.name, True, get_text_color())
                    ver_s    = small_font.render(f"v{mod.version}", True, (150,150,170))
                    status_s = small_font.render(status_str, True, status_col)
                    for s in (name_s, ver_s, status_s): s.set_alpha(alpha)
                    base_surface.blit(name_s,   (72,  y+7))
                    base_surface.blit(ver_s,    (280, y+7))
                    base_surface.blit(status_s, (WIDTH-140, y+7))
                    y += 36
            else:
                no_mods_s = small_font.render("Keine Mods geladen.", True, (160,160,160))
                base_surface.blit(no_mods_s, (WIDTH//2 - no_mods_s.get_width()//2, y+10))

        # Zurück-Button am unteren Rand (immer sichtbar)
        back_rect = pygame.Rect(WIDTH - 180, HEIGHT - 60, 150, 40)
        pygame.draw.rect(base_surface, get_button_color(), back_rect, border_radius=10)
        pygame.draw.rect(base_surface, (180, 180, 180), back_rect, width=2, border_radius=10)
        back_label = small_font.render(get_text("back"), True, get_text_color())
        base_surface.blit(back_label, (back_rect.centerx - back_label.get_width() // 2, back_rect.centery - back_label.get_height() // 2))
        stat_tab_rects.append((back_rect, -1))  # Index -1 für Zurück-Funktion

        # Bildschirm aktualisieren
        pygame.display.flip()

    except Exception as e:
        # Fehlerbehandlung: Bei Problemen zurück zum Hauptmenü
        print("[Fehler] draw_statistics():", e)
        showing_stats = False    # Statistiken-Bildschirm verlassen
        in_profile_hub = True    # Zurück zum Profil-Hub aktivieren


# =============================
# Shop (Lootbox-Style: endless scrolling ribbon) + Roll x10
# =============================

SHOP_COST_SINGLE = 50
SHOP_COST_X10 = 500
DUPLICATE_REFUND = 10

# Precompute list of all powers (stable order)
POWERS_LIST = list(POWERS_DB.keys())

def roll_power():
    # Weighted rarity → pick power
    r = random.randint(1, sum(RARITY_PROBS.values()))
    upto = 0
    chosen_rarity = "Common"
    for rar, p in RARITY_PROBS.items():
        upto += p
        if r <= upto:
            chosen_rarity = rar
            break
    candidates = [name for name, d in POWERS_DB.items() if d["rarity"] == chosen_rarity]
    return random.choice(candidates) if candidates else None

# --- Shop state (ribbon)
ribbon_tiles = []        # list[str] power names (base pattern)
tile_w, tile_h = 150, 130
ribbon_y = 300
scroll_offset = 0.0
spin_animating = False
spin_start_time = 0
spin_duration_ms = 2200
spin_final_offset = 0.0
spin_target_power = None
results_overlay = None  # list of tuples: (name, rarity, is_duplicate, coin_change)

# overlay messages (text overlays displayed in-game / shop)
overlays = []  # list of dicts: {"text": str, "color": (r,g,b), "expires": timestamp, "pos": (x,y) or None}
flash_timer = 0  # for screen flash effects
_trail_anim_t = 0.0  # animation phase for cosmetic shop trail preview

def push_overlay(text, color=WHITE, duration=1.5, pos=None):
    # Wenn der Text ein Key ist, versuche ihn zu übersetzen
    if text in texts.get(languages[language_index], {}):
        text = get_text(text)
    overlays.append({"text": text, "color": color, "expires": time.time() + duration, "pos": pos})

def cleanup_overlays():
    now = time.time()
    while overlays and overlays[0]["expires"] <= now:
        overlays.pop(0)
    # also remove any expired anywhere in list
    overlays[:] = [o for o in overlays if o["expires"] > now]

# achievement popups — animated card (bottom-right corner, separate from overlays)
achievement_popup_queue = []
_ACHIEVEMENT_POPUP_DURATION = 3.5

def push_achievement_popup(title, coins=0):
    achievement_popup_queue.append({"title": title, "coins": coins, "start_time": time.time()})

def draw_achievement_popups(surface):
    import math as _math
    now = time.time()
    achievement_popup_queue[:] = [p for p in achievement_popup_queue if now - p["start_time"] < _ACHIEVEMENT_POPUP_DURATION]
    if not achievement_popup_queue:
        return
    popup = achievement_popup_queue[0]
    age = now - popup["start_time"]
    slide_in = min(1.0, age / 0.3)
    fade_t = _ACHIEVEMENT_POPUP_DURATION - 0.5
    fade_out = max(0.0, 1.0 - (age - fade_t) / 0.5) if age > fade_t else 1.0
    t_factor = slide_in * fade_out

    pw, ph = 300, 72
    px = int(WIDTH - pw - 12 + (1.0 - t_factor) * (pw + 20))
    py = HEIGHT - ph - 60

    glow_v = int(200 + 55 * _math.sin(now * 5))
    glow_col = (glow_v, int(glow_v * 0.78), 30)

    pygame.draw.rect(surface, (22, 18, 40), pygame.Rect(px, py, pw, ph), border_radius=10)
    pygame.draw.rect(surface, (int(glow_v * 0.4), int(glow_v * 0.3), 10),
                     pygame.Rect(px - 3, py - 3, pw + 6, ph + 6), width=2, border_radius=13)
    pygame.draw.rect(surface, glow_col, pygame.Rect(px, py, pw, ph), width=2, border_radius=10)

    hdr = small_font.render("Achievement Unlocked!", True, (255, 195, 50))
    surface.blit(hdr, (px + 10, py + 9))

    title_s = small_font.render(popup["title"], True, (220, 220, 255))
    surface.blit(title_s, (px + 10, py + 38))

    if popup.get("coins", 0) > 0:
        coins_s = small_font.render(f"+{popup['coins']} Coins", True, (255, 220, 60))
        surface.blit(coins_s, (px + pw - coins_s.get_width() - 10, py + 38))


def draw_ingame_achievement_banner(surface):
    import math as _math
    now = time.time()
    if not achievement_popup_queue:
        return
    popup = achievement_popup_queue[0]
    age = now - popup["start_time"]
    if age >= _ACHIEVEMENT_POPUP_DURATION:
        return
    slide_in = min(1.0, age / 0.25)
    fade_t = _ACHIEVEMENT_POPUP_DURATION - 0.6
    fade_out = max(0.0, 1.0 - (age - fade_t) / 0.6) if age > fade_t else 1.0
    alpha = int(255 * slide_in * fade_out)
    if alpha <= 0:
        return

    bw, bh = 340, 58
    bx = WIDTH // 2 - bw // 2
    by = int(8 + (1.0 - slide_in) * (-bh - 12))

    glow_v = int(200 + 55 * _math.sin(now * 5))
    glow_col = (glow_v, int(glow_v * 0.78), 30)

    banner = pygame.Surface((bw, bh), pygame.SRCALPHA)
    banner.fill((22, 18, 40, min(230, alpha)))
    pygame.draw.rect(banner, (*glow_col, alpha), pygame.Rect(0, 0, bw, bh), width=2, border_radius=10)

    hdr = small_font.render("Achievement Unlocked!", True, (255, 195, 50))
    hdr.set_alpha(alpha)
    banner.blit(hdr, (bw // 2 - hdr.get_width() // 2, 6))

    title_s = small_font.render(popup["title"], True, (220, 220, 255))
    title_s.set_alpha(alpha)
    t_x = bw // 2 - title_s.get_width() // 2
    if popup.get("coins", 0) > 0:
        coins_s = small_font.render(f"+{popup['coins']}", True, (255, 220, 60))
        coins_s.set_alpha(alpha)
        t_x = max(10, bw // 2 - title_s.get_width() // 2 - coins_s.get_width() // 2 - 4)
        banner.blit(title_s, (t_x, 32))
        banner.blit(coins_s, (t_x + title_s.get_width() + 8, 32))
    else:
        banner.blit(title_s, (t_x, 32))

    surface.blit(banner, (bx, by))


# Buttons in shop
shop_btns = []  # (rect,label,cb)

def make_ribbon_with_win(power_name, base_tiles=40):
    """Build a base ribbon tile sequence (small set), and compute a win index so that we can
    produce an effectively endless/looping scroll by wrapping draw calls. We will compute
    a target offset (in px) that centers a chosen tile. Instead of building a huge list,
    we'll repeat the base pattern virtually."""
    global ribbon_tiles
    # Build weighted list of rarities
    rarities = []
    for rar, p in RARITY_PROBS.items():
        rarities += [rar] * p
    # Helper to sample a power by rarity
    def sample_power_by_rarity(rar):
        candidates = [n for n, d in POWERS_DB.items() if d["rarity"] == rar]
        return random.choice(candidates) if candidates else random.choice(POWERS_LIST)

    tiles = []
    for _ in range(base_tiles):
        rar = random.choice(rarities)
        tiles.append(sample_power_by_rarity(rar))

    # Ensure power_name included at least once; force one random position to be the power_name
    insert_idx = random.randint(6, max(6, base_tiles-6))
    tiles[insert_idx] = power_name

    ribbon_tiles = tiles
    # compute a "virtual" win_index that we'll use as the primary centered occurrence
    # We'll place that occurrence at many loops later when animating; returning base index is enough.
    return insert_idx

# Rarity-Farbschema (bg/border/badge getrennt, wie im Cosmetic Shop) statt einer Flachfarbe
POWER_RARITY_COLORS = {
    "Common":    {"bg": (40, 110, 40),  "border": (100, 220, 100), "badge": (70, 180, 70)},
    "Rare":      {"bg": (30, 70, 160),  "border": (90, 150, 255),  "badge": (60, 120, 220)},
    "Epic":      {"bg": (90, 30, 150),  "border": (190, 90, 255),  "badge": (150, 60, 230)},
    "Legendary": {"bg": (150, 100, 0),  "border": (255, 210, 60),  "badge": (220, 170, 20)},
    "Mythic":    {"bg": (170, 70, 0),   "border": (255, 150, 40),  "badge": (230, 110, 10)},
}
RARITY_ORDER = ["Mythic", "Legendary", "Epic", "Rare", "Common"]  # seltenste zuerst
_RARITY_ICON_SHAPE = {"Common": "circle", "Rare": "diamond", "Epic": "hexagon",
                       "Legendary": "star4", "Mythic": "star4"}


def _draw_rarity_icon(surface, shape, color, icx, icy, r=9):
    """Kleine Vektor-Icons je Rarity-Tier (kein Bild-Asset nötig)."""
    if shape == "circle":
        pygame.draw.circle(surface, color, (icx, icy), r)
    elif shape == "diamond":
        pygame.draw.polygon(surface, color, [(icx, icy-r), (icx+r, icy), (icx, icy+r), (icx-r, icy)])
    elif shape == "hexagon":
        pts = [(icx + int(r * math.cos(math.radians(60*n - 30))),
                icy + int(r * math.sin(math.radians(60*n - 30)))) for n in range(6)]
        pygame.draw.polygon(surface, color, pts)
    elif shape == "star4":
        rr = r * 1.25
        pts = [(icx, icy-rr), (icx+rr*0.36, icy-rr*0.36), (icx+rr, icy),
               (icx+rr*0.36, icy+rr*0.36), (icx, icy+rr), (icx-rr*0.36, icy+rr*0.36),
               (icx-rr, icy), (icx-rr*0.36, icy-rr*0.36)]
        pygame.draw.polygon(surface, color, pts)


def draw_ribbon_shop():
    dark = is_dark_mode()
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    # Dezente Deko-Kreise im Hintergrund (wie Cosmetic Shop)
    deco_colors = [(200, 210, 255), (220, 200, 255)] if not dark else [(60, 80, 140), (80, 50, 120)]
    for i, dc in enumerate(deco_colors):
        s = pygame.Surface((300, 300), pygame.SRCALPHA)
        pygame.draw.circle(s, dc + (40,), (150, 150), 150)
        base_surface.blit(s, (i * 500 - 100, 40))

    # Titel mit Schatten
    title_text = get_text("shop") + " - " + get_text("rollen")
    title_shadow = font.render(title_text, True, (80, 80, 100))
    title_surf = font.render(title_text, True, get_text_color())
    base_surface.blit(title_shadow, (WIDTH // 2 - title_surf.get_width() // 2 + 2, 22))
    base_surface.blit(title_surf, (WIDTH // 2 - title_surf.get_width() // 2, 20))

    # Coin-Anzeige mit Icon+Panel (statt nacktem Text)
    coin_bg = pygame.Rect(20, 15, 150, 32)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=16)
    pygame.draw.rect(base_surface, (200, 180, 60), coin_bg, width=2, border_radius=16)
    pygame.draw.circle(base_surface, GOLD, (coin_bg.x + 18, coin_bg.centery), 8)
    pygame.draw.circle(base_surface, (180, 140, 0), (coin_bg.x + 18, coin_bg.centery), 8, width=2)
    coin_text = small_font.render(f"{stats.get('coins', 0)}", True, GOLD)
    base_surface.blit(coin_text, (coin_bg.x + 32, coin_bg.y + 7))

    # Klickbarer Back-Button oben rechts (gab's bisher nicht, nur ESC/M)
    back_rect = pygame.Rect(WIDTH - 130, 15, 110, 32)
    back_hov = back_rect.collidepoint(mouse_pos) or nav_is_focused("shop", len(shop_btns), len(shop_btns) + 1)
    pygame.draw.rect(base_surface, get_button_hover_color() if back_hov else get_button_color(), back_rect, border_radius=16)
    pygame.draw.rect(base_surface, (160, 160, 160), back_rect, width=2 if not back_hov else 3, border_radius=16)
    back_lbl = small_font.render(get_text("back"), True, get_text_color())
    base_surface.blit(back_lbl, (back_rect.centerx - back_lbl.get_width() // 2, back_rect.centery - back_lbl.get_height() // 2))

    hint = small_font.render("SPACE: " + get_text("spin") + "  |  X: " + get_text("spin10") + "  |  M/ESC: Menü", True,
                              (150, 150, 150) if dark else (110, 110, 110))
    base_surface.blit(hint, (WIDTH//2 - hint.get_width()//2, HEIGHT-30))

    # Sichtbares Fenster (mit Schatten dahinter für mehr Tiefe)
    window_rect = pygame.Rect(40, ribbon_y - tile_h//2, WIDTH-80, tile_h)
    shadow_rect = pygame.Rect(window_rect.x + 4, window_rect.y + 4, window_rect.w, window_rect.h)
    pygame.draw.rect(base_surface, (0, 0, 0, 60) if dark else (180, 180, 200), shadow_rect, border_radius=14)
    pygame.draw.rect(base_surface, get_panel_color(), window_rect, border_radius=14)
    pygame.draw.rect(base_surface, (60, 80, 140), window_rect, width=2, border_radius=14)

    if not ribbon_tiles:
        return

    total_base = len(ribbon_tiles)
    total_width = total_base * tile_w
    base_offset = scroll_offset % total_width
    x0 = window_rect.x - int(base_offset)

    now = time.time()
    glow_v = int(200 + 55 * math.sin(now * 5))  # gleicher Puls wie Achievement-Popups
    glow_col = (glow_v, int(glow_v * 0.78), 30)
    cx = WIDTH // 2

    for i in range(-total_base, total_base*2):
        idx = i % total_base
        name = ribbon_tiles[idx]
        x = x0 + i * tile_w
        rect = pygame.Rect(x, window_rect.y + 4, tile_w-8, tile_h-8)
        if rect.right < window_rect.left or rect.left > window_rect.right:
            continue
        rar = POWERS_DB.get(name, {}).get("rarity", "Common")
        rc = POWER_RARITY_COLORS.get(rar, POWER_RARITY_COLORS["Common"])
        is_center = rect.left <= cx <= rect.right

        card_bg = tuple(max(0, c - 40) for c in rc["bg"]) if dark else rc["bg"]
        pygame.draw.rect(base_surface, card_bg, rect, border_radius=10)
        # Zentrale Kachel: Rand pulst bei Legendary/Mythic golden, sonst normaler Rarity-Rand
        if is_center and rar in ("Legendary", "Mythic"):
            pygame.draw.rect(base_surface, glow_col, rect, width=3, border_radius=10)
        elif is_center:
            pygame.draw.rect(base_surface, rc["border"], rect, width=3, border_radius=10)
        else:
            pygame.draw.rect(base_surface, tuple(max(0, c - 60) for c in rc["border"]), rect, width=1, border_radius=10)

        # Icon-Medaillon oben, Name darunter
        icx, icy = rect.centerx, rect.y + 28
        pygame.draw.circle(base_surface, tuple(min(255, c + 40) for c in card_bg), (icx, icy), 18)
        pygame.draw.circle(base_surface, rc["border"], (icx, icy), 18, width=2)
        _draw_rarity_icon(base_surface, _RARITY_ICON_SHAPE.get(rar, "circle"), rc["border"], icx, icy, r=9)

        label = _fit_surf(menu_font.render(get_power_name(name), True, (245, 245, 250)), rect.width - 8)
        base_surface.blit(label, (rect.centerx - label.get_width()//2, rect.y + 58))
        rar_s = small_font.render(rar, True, tuple(min(255, c+60) for c in rc["border"]))
        base_surface.blit(rar_s, (rect.centerx - rar_s.get_width()//2, rect.bottom - 26))

    # Center-Pointer mit leichtem Glow
    pygame.draw.polygon(base_surface, glow_col, [(cx, window_rect.bottom + 18), (cx-10, window_rect.bottom-2), (cx+10, window_rect.bottom-2)])
    pygame.draw.line(base_surface, glow_col, (cx, window_rect.top-6), (cx, window_rect.bottom+18), 2)

    # Buttons (2-zeilig: Label + Kosten, echter Hover-Farbwechsel, gedimmt bei zu wenig Coins)
    coins_now = stats.get("coins", 0)
    for _sbi, (rect, label, cost, _) in enumerate(shop_btns):
        affordable = coins_now >= cost
        hov = (rect.collidepoint(mouse_pos) or nav_is_focused("shop", _sbi, len(shop_btns) + 1)) and affordable
        if not affordable:
            btn_bg = (50, 50, 55) if dark else (210, 210, 210)
        elif hov:
            btn_bg = (0, 150, 90)
        else:
            btn_bg = (0, 115, 70)
        pygame.draw.rect(base_surface, btn_bg, rect, border_radius=12)
        pygame.draw.rect(base_surface, (0, 200, 120) if (affordable and hov) else (150, 150, 150), rect,
                         width=3 if hov else 2, border_radius=12)
        lab_col = (255, 255, 255) if affordable else (140, 140, 140)
        lab = small_font.render(label, True, lab_col)
        base_surface.blit(lab, (rect.centerx - lab.get_width()//2, rect.y + 8))
        cost_col = (255, 220, 100) if affordable else (200, 90, 90)
        cost_s = small_font.render(f"{cost} Coins", True, cost_col)
        base_surface.blit(cost_s, (rect.centerx - cost_s.get_width()//2, rect.y + 32))

    # If overlay with results (for x10) is active
    if results_overlay is not None:
        draw_results_overlay(results_overlay)

    pygame.display.flip()

def draw_results_overlay(items):
    dark = is_dark_mode()

    # Semi-transparent backdrop
    s = pygame.Surface((WIDTH, HEIGHT))
    s.set_alpha(200)
    s.fill(OVERLAY_BG)
    base_surface.blit(s, (0,0))

    # Panel mit Schatten
    panel = pygame.Rect(80, 80, WIDTH-160, HEIGHT-160)
    shadow = pygame.Rect(panel.x + 5, panel.y + 5, panel.w, panel.h)
    pygame.draw.rect(base_surface, (0, 0, 0, 80) if dark else (170, 170, 190), shadow, border_radius=16)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=16)
    pygame.draw.rect(base_surface, (60, 80, 140), panel, width=3, border_radius=16)

    title_text = get_text("results")
    title_shadow = menu_font.render(title_text, True, (80, 80, 100))
    title = menu_font.render(title_text, True, get_text_color())
    base_surface.blit(title_shadow, (panel.centerx - title.get_width()//2 + 2, panel.y + 18))
    base_surface.blit(title, (panel.centerx - title.get_width()//2, panel.y + 16))

    # Zeilen: seltenste zuerst, mit Rarity-Badge + Icon statt Farbquadrat
    sorted_items = sorted(items, key=lambda it: RARITY_ORDER.index(it[1]) if it[1] in RARITY_ORDER else len(RARITY_ORDER))
    row_h = 34
    y = panel.y + 60
    for name, rarity, is_dup, delta in sorted_items:
        rc = POWER_RARITY_COLORS.get(rarity, POWER_RARITY_COLORS["Common"])
        row_rect = pygame.Rect(panel.x + 16, y, panel.w - 32, row_h - 6)
        row_bg = tuple(max(0, c - 65) for c in rc["bg"]) if dark else tuple(min(255, c + 130) for c in rc["bg"])
        pygame.draw.rect(base_surface, row_bg, row_rect, border_radius=8)
        pygame.draw.rect(base_surface, rc["border"], row_rect, width=1, border_radius=8)

        icx, icy = row_rect.x + 20, row_rect.centery
        pygame.draw.circle(base_surface, rc["bg"], (icx, icy), 11)
        pygame.draw.circle(base_surface, rc["border"], (icx, icy), 11, width=2)
        _draw_rarity_icon(base_surface, _RARITY_ICON_SHAPE.get(rarity, "circle"), rc["border"], icx, icy, r=5)

        badge_text = small_font.render(rarity, True, (255, 255, 255))
        badge_rect = pygame.Rect(row_rect.x + 40, row_rect.centery - 10, badge_text.get_width() + 12, 20)
        pygame.draw.rect(base_surface, rc["badge"], badge_rect, border_radius=10)
        base_surface.blit(badge_text, (badge_rect.x + 6, badge_rect.y + 2))

        label = get_power_name(name)
        if is_dup:
            label += f"  (Duplikat +{delta} Coins)"
        txt = _fit_surf(small_font.render(label, True, get_text_color()), row_rect.right - badge_rect.right - 16)
        base_surface.blit(txt, (badge_rect.right + 10, row_rect.centery - txt.get_height()//2))
        y += row_h

    close_rect = pygame.Rect(panel.centerx - 110, panel.bottom - 44, 220, 30)
    close_hov = close_rect.collidepoint(get_mouse_pos_logical())
    pygame.draw.rect(base_surface, get_button_hover_color() if close_hov else get_button_color(), close_rect, border_radius=10)
    pygame.draw.rect(base_surface, (150, 150, 150), close_rect, width=2, border_radius=10)
    close_hint = small_font.render(get_text("enter_click_close_hint"), True, get_text_color())
    base_surface.blit(close_hint, (close_rect.centerx - close_hint.get_width()//2, close_rect.centery - close_hint.get_height()//2))

def start_single_spin():
    global spin_animating, spin_start_time, spin_final_offset, spin_target_power, scroll_offset
    if spin_animating or results_overlay is not None:
        return
    if stats.get("coins",0) < SHOP_COST_SINGLE:
        push_overlay("insufficient_coins", (200,0,0))
        return
    # Deduct immediately
    stats["coins"] -= SHOP_COST_SINGLE
    save_section("stats", stats)

    # Decide power via roll_power(); build ribbon with this power and compute target offset
    power = roll_power()
    spin_target_power = power
    base_index = make_ribbon_with_win(power, base_tiles=40)
    # We will animate over several "virtual loops" to the occurrence at base_index in some future loop.
    total_base = len(ribbon_tiles)
    total_width = total_base * tile_w

    # choose number of loops to advance (2..4)
    extra_loops = random.randint(2, 4)
    # compute target offset such that the chosen tile (occurrence at base_index in loop N) is centered
    center_x = (WIDTH // 2) - (tile_w // 2)
    # target base pixel of the chosen tile in its loop:
    target_base_px = (extra_loops * total_width) + (base_index * tile_w)
    spin_final_offset = target_base_px - center_x
    # ensure scroll_offset is current modulo range to keep animation consistent
    scroll_offset = scroll_offset % total_width
    # start animation
    spin_animating = True
    spin_start_time = pygame.time.get_ticks()

def grant_spin_reward(name):
    # Called when single spin animation stops
    if name in powers_state["owned"]:
        stats["coins"] += DUPLICATE_REFUND
        save_section("stats", stats)
        push_overlay(f"{get_text('duplicate')}: +{DUPLICATE_REFUND} {get_text('coins')}", (60,60,60))
    else:
        powers_state["owned"].append(name)
        if name == "Extra Slot":
            push_overlay("extra_slot_unlocked", GOLD)
        else:
            # Auto-equip: if there is room, equip automatically; if not, follow auto-swap logic
            if len(powers_state["equipped"]) < get_max_slots():
                powers_state["equipped"].append(name)
            else:
                # replace oldest equipped automatically
                if powers_state["equipped"]:
                    dropped = powers_state["equipped"].pop(0)
                    powers_state["equipped"].append(name)
                    push_overlay(f"{get_text('auto_swap')}: {dropped} → {name}", (180,140,20))
                else:
                    powers_state["equipped"].append(name)
        save_section("powers", powers_state)
        push_overlay(f"{get_text('new_power')}: {get_power_name(name)}", GOLD)

def do_roll_x10():
    global results_overlay
    if spin_animating:
        return
    if stats.get("coins",0) < SHOP_COST_X10:
        push_overlay("Zu wenig Coins!", (200,0,0))
        return
    stats["coins"] -= SHOP_COST_X10
    save_section("stats", stats)

    items = []
    coin_back = 0
    for _ in range(10):
        p = roll_power()
        rarity = POWERS_DB[p]["rarity"]
        if p in powers_state["owned"]:
            coin_back += DUPLICATE_REFUND
            items.append((p, rarity, True, DUPLICATE_REFUND))
        else:
            powers_state["owned"].append(p)
            if p == "Extra Slot":
                items.append((p, rarity, False, 0))
                push_overlay("Extra Slot freigeschaltet! (passiv)", GOLD)
                continue
            # equip if space else auto-swap oldest
            if len(powers_state["equipped"]) < get_max_slots():
                powers_state["equipped"].append(p)
            else:
                if powers_state["equipped"]:
                    powers_state["equipped"].pop(0)
                    powers_state["equipped"].append(p)
            items.append((p, rarity, False, 0))

    if coin_back:
        stats["coins"] += coin_back
    save_section("stats", stats)
    save_section("powers", powers_state)

    results_overlay = items  # show overlay; closes on ENTER/click
    push_overlay(f"Roll x10: +{coin_back} Coins zurück" if coin_back else "Roll x10 abgeschlossen", (80,160,80))

def shop_loop():
    global in_shop, in_menu, spin_animating, scroll_offset, results_overlay, spin_final_offset, spin_target_power
    # Build buttons: left single, right x10
    def _make_buttons():
        return [
            (pygame.Rect(WIDTH//2 - 220, 460, 200, 56), get_text("spin"), SHOP_COST_SINGLE, start_single_spin),
            (pygame.Rect(WIDTH//2 + 20, 460, 200, 56), get_text("spin10"), SHOP_COST_X10, do_roll_x10),
        ]
    global shop_btns
    shop_btns = _make_buttons()

    # Ensure there is a default ribbon when opening shop
    if not ribbon_tiles:
        make_ribbon_with_win(random.choice(POWERS_LIST), base_tiles=40)

    while in_shop:
        dt_ms = 16
        # Update animation state
        if spin_animating:
            t = pygame.time.get_ticks() - spin_start_time
            if t >= spin_duration_ms:
                # End precisely: align so the target tile is centered - no post-slip
                spin_animating = False
                # Align scroll_offset exactly to final (use float but keep stable)
                scroll_offset = spin_final_offset
                # grant reward
                if spin_target_power:
                    grant_spin_reward(spin_target_power)
                    spin_target_power = None
            else:
                # Ease-out scrolling (smooth)
                k = t / spin_duration_ms  # 0..1
                ease = 1 - (1 - k)**3
                current = ease * spin_final_offset
                scroll_offset = current

        # clean overlays
        cleanup_overlays()

        draw_ribbon_shop()
        _shop_rects = [r for r, _l, _c, _cb in shop_btns] + [pygame.Rect(WIDTH - 130, 15, 110, 32)]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if results_overlay is not None:
                    if is_nav_confirm(event.key) or is_back_key(event.key):
                        results_overlay = None
                elif event.key == pygame.K_m or is_back_key(event.key):
                    in_shop = False
                    in_menu = True
                elif event.key == pygame.K_x:
                    do_roll_x10()
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("shop", len(_shop_rects))
                        nav_set_focus("shop", nav_move(_shop_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("shop", len(_shop_rects))
                        if _fi is not None and _fi < len(shop_btns):
                            shop_btns[_fi][3]()
                        elif _fi == len(shop_btns):
                            in_shop = False
                            in_menu = True
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                if results_overlay is not None:
                    results_overlay = None
                else:
                    back_rect = pygame.Rect(WIDTH - 130, 15, 110, 32)
                    if back_rect.collidepoint((mx, my)):
                        play_sound(click_back_sound)
                        in_shop = False
                        in_menu = True
                    else:
                        for rect, _, cost, cb in shop_btns:
                            if rect.collidepoint((mx, my)):
                                cb()
                                break

        # draw overlays (non-blocking)
        y_ov = 12
        for ov in overlays:
            txt = small_font.render(ov["text"], True, ov["color"])
            base_surface.blit(txt, ((WIDTH - txt.get_width()) // 2, y_ov))
            y_ov += txt.get_height() + 4
        draw_achievement_popups(base_surface)
        pygame.display.flip()

        pygame.time.delay(dt_ms)


def to_power_loadout():
    global in_loadout_menu, in_loadout
    in_loadout_menu = False
    in_loadout = True
    play_sound(menu_click_sound, "menu_click_volume")


_cosmetic_inventory_return_screen = "in_loadout_menu"  # welcher Screen-Flag beim Zurück-Klick aktiviert wird

def _cosmetic_inventory_back():
    """Verlässt das Kosmetik-Inventar zu dem Screen, von dem aus es geöffnet wurde
    (Loadout-Hub oder Profil-Hub, siehe _cosmetic_inventory_return_screen)."""
    global in_cosmetic_inventory
    in_cosmetic_inventory = False
    globals()[_cosmetic_inventory_return_screen] = True

def to_cosmetic_inventory():
    global in_loadout_menu, in_cosmetic_inventory, _cosmetic_inventory_return_screen
    in_loadout_menu = False
    in_cosmetic_inventory = True
    _cosmetic_inventory_return_screen = "in_loadout_menu"



def draw_loadout_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    title = font.render(get_text("loadout"), True, get_text_color())
    base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 70))
    
    # Coin-Anzeige
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    
    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))
    
    # Buttons
    start_x = WIDTH//2 - 180
    start_y = 180
    gap = 70
    power_loadout_rect = pygame.Rect(start_x, start_y + 0*gap, 360, 48)
    cosmetic_rect = pygame.Rect(start_x, start_y + 1*gap, 360, 48)
    back_rect = pygame.Rect(start_x, start_y + 2*gap, 360, 48)
    
    _lm_labels = [(power_loadout_rect, "loadout"), (cosmetic_rect, "cosmetic_inventory"), (back_rect, "back")]
    for i, (rect, text_key) in enumerate(_lm_labels):
        is_hovered = rect.collidepoint(mouse_pos) or nav_is_focused("loadout_menu", i, len(_lm_labels))
        color = get_button_hover_color() if is_hovered else get_button_color()
        pygame.draw.rect(base_surface, color, rect, border_radius=12)
        pygame.draw.rect(base_surface, (180, 180, 180), rect, width=3 if is_hovered else 2, border_radius=12)

        text_surf = menu_font.render(get_text(text_key), True, get_text_color())
        base_surface.blit(text_surf, (rect.centerx - text_surf.get_width() // 2,
                                rect.centery - text_surf.get_height() // 2))

    pygame.display.flip()


def loadout_menu_loop():
    global in_loadout_menu, in_menu
    while in_loadout_menu:
        start_x = WIDTH//2 - 180
        start_y = 180
        gap = 70
        power_loadout_rect = pygame.Rect(start_x, start_y + 0*gap, 360, 48)
        cosmetic_rect = pygame.Rect(start_x, start_y + 1*gap, 360, 48)
        back_rect = pygame.Rect(start_x, start_y + 2*gap, 360, 48)
        _lm_rects = [power_loadout_rect, cosmetic_rect, back_rect]
        _lm_actions = ["power_loadout", "cosmetic_inventory", "back"]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit()
                sys.exit()
            _lm_action = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    _lm_action = "back"
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("loadout_menu", len(_lm_rects))
                        nav_set_focus("loadout_menu", nav_move(_lm_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _lm_action = _lm_actions[nav_get_focus("loadout_menu", len(_lm_rects))]
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mouse_pos = get_mouse_pos_logical()
                mx, my = mouse_pos
                if power_loadout_rect.collidepoint((mx, my)):
                    _lm_action = "power_loadout"
                elif cosmetic_rect.collidepoint((mx, my)):
                    _lm_action = "cosmetic_inventory"
                elif back_rect.collidepoint((mx, my)):
                    _lm_action = "back"
            if _lm_action == "power_loadout":
                to_power_loadout()
            elif _lm_action == "cosmetic_inventory":
                to_cosmetic_inventory()
            elif _lm_action == "back":
                in_loadout_menu = False
                in_menu = True
        draw_loadout_menu()
        clock.tick(60)



# Reihenfolge & Zuordnung der 4 Kosmetik-Tabs im Inventar
# 0 = Spuren (trail), 1 = Kopfbedeckungen (headwear), 2 = Gesichtsmerkmale (face,
# mit Unterkategorien Augen/Nase/Mund), 3 = Tools (coming soon)
COSMETIC_TAB_CATEGORIES = ["trail", "headwear", "face", None]
COSMETIC_TAB_LABEL_KEYS = ["trails_tab", "headwear_tab", "face_tab", "tools_tab"]

# Unterkategorien des "Gesichtsmerkmale"-Tabs. Jede hat weiterhin ihren eigenen,
# unabhängigen Ausrüstungs-Slot (equipped_eyes / equipped_nose / equipped_mouth),
# nur die Tab-Navigation wurde zusammengefasst.
FACE_SUBCATEGORIES = ["eyes", "nose", "mouth"]
FACE_SUBCATEGORY_LABEL_KEYS = ["eyes_tab", "nose_tab", "mouth_tab"]
face_subcategory = "eyes"  # aktuell gewählte Unterkategorie im Gesichtsmerkmale-Tab

def resolve_cosmetic_category(tab):
    """Löst einen Tab-Index in die tatsächliche Item-Kategorie auf. Beim
    'face'-Tab (Gesichtsmerkmale) bestimmt die aktuell gewählte Unterkategorie
    (face_subcategory) die echte Kategorie (eyes/nose/mouth)."""
    category = COSMETIC_TAB_CATEGORIES[tab]
    if category == "face":
        return face_subcategory
    return category

def get_face_subtab_layout():
    """Position/Größe der 3 Unterkategorie-Buttons (Augen/Nase/Mund), die nur
    innerhalb des 'Gesichtsmerkmale'-Haupttabs angezeigt werden."""
    count = len(FACE_SUBCATEGORIES)
    gap = 6
    h = 30
    w = min(150, (WIDTH - 80 - (count - 1) * gap) // count)
    x_start = WIDTH // 2 - (count * w + (count - 1) * gap) // 2
    y = 146
    return [pygame.Rect(x_start + i * (w + gap), y, w, h) for i in range(count)]

def get_cosmetic_tab_layout(reserve_sale=False):
    """Berechnet Position/Größe der Kosmetik-Tabs anhand der aktuellen Tab-Anzahl
    (COSMETIC_TAB_CATEGORIES). Zentrale Stelle für Zeichnen UND Klick-Erkennung,
    damit beide bei einer wachsenden Tab-Anzahl nie auseinanderlaufen und die
    Tab-Leiste immer auf den Bildschirm passt.
    reserve_sale=True (nur im Kosmetik-Shop): reserviert links von den Tabs Platz
    für den Sale-Button, damit beide Gruppen zusammen zentriert sind, statt dass
    der Sale-Button in die Tabs hineinragt oder der freie Platz fehlt."""
    tab_count = len(COSMETIC_TAB_CATEGORIES)
    tab_gap = 6
    tab_h = 40
    margin = 40  # Rand links/rechts, damit die Leiste nie über den Bildschirmrand ragt
    sale_w, sale_gap = _SALE_BUTTON_W, _SALE_BUTTON_GAP
    reserved = (sale_w + sale_gap) if reserve_sale else 0
    tab_w = min(172, (WIDTH - margin - reserved - (tab_count - 1) * tab_gap) // tab_count)
    tabs_total_w = tab_count * tab_w + (tab_count - 1) * tab_gap
    y_tabs = 100
    if reserve_sale:
        group_w = sale_w + sale_gap + tabs_total_w
        tabs_x_start = WIDTH // 2 - group_w // 2 + sale_w + sale_gap
    else:
        tabs_x_start = WIDTH // 2 - tabs_total_w // 2
    tab_rects = [pygame.Rect(tabs_x_start + i * (tab_w + tab_gap), y_tabs, tab_w, tab_h) for i in range(tab_count)]
    return tab_rects, tab_w, tab_h

_SALE_BUTTON_W = 100
_SALE_BUTTON_GAP = 12

def get_sale_button_rect():
    """Position/Größe des roten Sale-Buttons im Kosmetik-Shop - direkt links
    neben dem ersten Kategorie-Tab (Spuren), auf gleicher Höhe. Wird von der
    Tab-Leiste selbst abgeleitet (reserve_sale=True), damit beide garantiert
    zusammenpassen und nie auseinanderlaufen."""
    tab_rects, _tab_w, tab_h = get_cosmetic_tab_layout(reserve_sale=True)
    first_tab = tab_rects[0]
    return pygame.Rect(first_tab.x - _SALE_BUTTON_GAP - _SALE_BUTTON_W, first_tab.y, _SALE_BUTTON_W, tab_h)

def get_cosmetics_for_tab(tab):
    """Gibt (gefilterte_owned_liste, aktuell_ausgerüstetes_item) für den gegebenen Tab-Index zurück."""
    category = resolve_cosmetic_category(tab)
    owned = cosmetics_state.get("owned", [])
    if category is None:  # Tools-Tab: noch keine Items
        return [], None
    filtered_owned = [c for c in owned if COSMETICS_DB.get(c, {}).get("category") == category]
    if category == "trail":
        equipped = cosmetics_state.get("equipped", [])
        active_equipped = equipped[0] if equipped else None
    else:
        # Jede Kategorie (headwear, face, ...) hat ihren eigenen, unabhängigen Slot
        # ("equipped_<category>"), damit z.B. eine Kopfbedeckung UND ein Gesichtsmerkmal
        # gleichzeitig ausgerüstet sein können - 1 Kosmetik-Item pro Tab/Kategorie.
        active_equipped = cosmetics_state.get(f"equipped_{category}")
    return filtered_owned, active_equipped

def equip_toggle_cosmetic(tab, cosmetic, active_equipped):
    """Rüstet ein Kosmetik-Item aus/an, je nach Tab-Kategorie. Jede Kategorie hat ihren
    eigenen Slot, sodass max. 1 Item pro Kategorie/Tab gleichzeitig ausgerüstet ist,
    aber verschiedene Kategorien sich nicht gegenseitig verdrängen."""
    category = resolve_cosmetic_category(tab)
    meta = COSMETICS_DB.get(cosmetic, {})
    name = meta.get("name", {}).get(languages[language_index], cosmetic)
    if active_equipped == cosmetic:
        if category == "trail":
            cosmetics_state['equipped'] = []
        else:
            cosmetics_state[f'equipped_{category}'] = None
        push_overlay(f"{name} unequipped", (180, 180, 180))
    else:
        if category == "trail":
            cosmetics_state['equipped'] = [cosmetic]
        else:
            cosmetics_state[f'equipped_{category}'] = cosmetic
        push_overlay(f"{name} equipped", (0, 200, 0))
    save_section("cosmetics", cosmetics_state)


def draw_cosmetic_inventory():
    global cosmetic_tab
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    title = font.render(get_text("cosmetic_inventory"), True, get_text_color())
    base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 20))

    # Tabs at top (4 Kategorien: Spuren, Kopfbedeckungen, Gesichtsmerkmale, Tools)
    tab_rects, tab_w, tab_h = get_cosmetic_tab_layout()
    tab_labels = [get_text(k) for k in COSMETIC_TAB_LABEL_KEYS]
    tab_label_font = tab_font if tab_w < 150 else small_font

    # Tabs zeichnen
    for i, (rect, label) in enumerate(zip(tab_rects, tab_labels)):
        if cosmetic_tab == i:
            bg_color, border_color = get_button_color(), get_button_hover_color()
        else:
            bg_color, border_color = get_panel_color(), (180, 180, 180)
        # Hintergrund
        pygame.draw.rect(base_surface, bg_color, rect, border_radius=12)
        # Border
        pygame.draw.rect(base_surface, border_color, rect, width=2, border_radius=12)
        # Text zentriert
        text_surf = tab_label_font.render(label, True, get_text_color())
        base_surface.blit(text_surf, (rect.centerx - text_surf.get_width() // 2,
                                rect.centery - text_surf.get_height() // 2))

    # Unterkategorie-Buttons (Augen/Nase/Mund) nur sichtbar, wenn der aktuelle
    # Haupt-Tab "Gesichtsmerkmale" ist. Verschiebt das Panel etwas nach unten,
    # damit genug Platz für die Buttons ist.
    is_face_tab = COSMETIC_TAB_CATEGORIES[cosmetic_tab] == "face"
    if is_face_tab:
        sub_rects = get_face_subtab_layout()
        sub_labels = [get_text(k) for k in FACE_SUBCATEGORY_LABEL_KEYS]
        for i, (srect, slabel) in enumerate(zip(sub_rects, sub_labels)):
            active = FACE_SUBCATEGORIES[i] == face_subcategory
            sbg = get_button_color() if active else get_panel_color()
            sborder = get_button_hover_color() if active else (150, 150, 150)
            pygame.draw.rect(base_surface, sbg, srect, border_radius=10)
            pygame.draw.rect(base_surface, sborder, srect, width=2, border_radius=10)
            stext = small_font.render(slabel, True, get_text_color())
            base_surface.blit(stext, (srect.centerx - stext.get_width() // 2,
                                      srect.centery - stext.get_height() // 2))
        panel_top = 186
    else:
        panel_top = 150

    # List owned cosmetics based on tab
    y = panel_top + 10
    panel = pygame.Rect(50, panel_top, WIDTH - 100, HEIGHT - panel_top - 60)
    pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=16)
    pygame.draw.rect(base_surface, (60, 80, 140), panel, width=3, border_radius=16)

    filtered_owned, active_equipped = get_cosmetics_for_tab(cosmetic_tab)

    # Kleiner Zähler oben rechts im Panel ("X besessen")
    if COSMETIC_TAB_CATEGORIES[cosmetic_tab] is not None:
        count_surf = tab_font.render(f"{len(filtered_owned)} {get_text('owned_tab')}", True, get_text_color())
        base_surface.blit(count_surf, (panel.right - count_surf.get_width() - 16, panel.y - 22))

    if COSMETIC_TAB_CATEGORIES[cosmetic_tab] is None:  # Tools - coming soon
        msg = small_font.render(get_text("tools_coming_soon"), True, get_text_color())
        base_surface.blit(msg, (panel.centerx - msg.get_width() // 2, panel.centery - msg.get_height() // 2))
    elif not filtered_owned:
        msg = small_font.render(get_text("no_owned"), True, get_text_color())
        base_surface.blit(msg, (panel.x + 20, y))
    else:
        for i, cosmetic in enumerate(filtered_owned):
            meta = COSMETICS_DB.get(cosmetic, {})
            rarity = meta.get("rarity", "Common")
            rc = get_rarity_colors(rarity)
            equipped = active_equipped == cosmetic

            # Item-Karte: dezenter Schatten + rarität-getönter linker Akzentbalken
            item_rect = pygame.Rect(panel.x + 20, y, panel.width - 40, 50)
            shadow_rect = item_rect.move(2, 2)
            pygame.draw.rect(base_surface, (0, 0, 0, 40) if is_dark_mode() else (210, 210, 225), shadow_rect, border_radius=10)
            pygame.draw.rect(base_surface, get_panel_color(), item_rect, border_radius=10)
            border_col = GREEN if equipped else rc["border"]
            pygame.draw.rect(base_surface, border_col, item_rect, width=2, border_radius=10)
            # Linker Akzentbalken in Raritätsfarbe
            accent_rect = pygame.Rect(item_rect.x, item_rect.y, 5, item_rect.height)
            pygame.draw.rect(base_surface, rc["border"], accent_rect, border_radius=6)

            # Icon in rarität-getöntem Kreis-Badge
            icx, icy = item_rect.x + 34, item_rect.centery
            pygame.draw.circle(base_surface, rc["bg"], (icx, icy), 19)
            pygame.draw.circle(base_surface, rc["border"], (icx, icy), 19, width=2)
            draw_cosmetic_icon(base_surface, cosmetic, icx, icy, big=False)

            # Name + kleine Raritäts-Pille
            name = meta.get("name", {}).get(languages[language_index], cosmetic)
            name_surf = small_font.render(name, True, get_text_color())
            base_surface.blit(name_surf, (item_rect.x + 62, item_rect.y + 8))
            badge_text = tab_font.render(rc["label"], True, (255, 255, 255))
            badge_rect = pygame.Rect(item_rect.x + 62, item_rect.y + 28, badge_text.get_width() + 12, 16)
            pygame.draw.rect(base_surface, rc["badge"], badge_rect, border_radius=8)
            base_surface.blit(badge_text, (badge_rect.x + 6, badge_rect.y + 1))

            # Equip/Unequip button (abgerundete Pille mit Häkchen bei Equipped)
            _ci_focused = nav_is_focused("cosmetic_inventory", i, len(filtered_owned) + 1)
            btn_rect = pygame.Rect(item_rect.right - 120, item_rect.y + 10, 100, 30)
            if equipped:
                btn_bg = (30, 170, 90) if not _ci_focused else (40, 200, 110)
                pygame.draw.rect(base_surface, btn_bg, btn_rect, border_radius=15)
                if _ci_focused:
                    pygame.draw.rect(base_surface, (255, 255, 255), btn_rect, width=2, border_radius=15)
                # Kleines Häkchen-Symbol
                pygame.draw.lines(base_surface, (255, 255, 255), False,
                                   [(btn_rect.x + 10, btn_rect.centery), (btn_rect.x + 15, btn_rect.centery + 5),
                                    (btn_rect.x + 24, btn_rect.centery - 6)], 2)
                unequip_text = small_font.render(get_text("unequip"), True, (255, 255, 255))
                base_surface.blit(unequip_text, (btn_rect.x + 30, btn_rect.centery - unequip_text.get_height() // 2))
            else:
                btn_bg = get_button_hover_color() if _ci_focused else get_button_color()
                pygame.draw.rect(base_surface, btn_bg, btn_rect, border_radius=15)
                pygame.draw.rect(base_surface, rc["border"] if not _ci_focused else (255, 255, 255),
                                  btn_rect, width=2, border_radius=15)
                equip_text = small_font.render(get_text("equip"), True, get_text_color())
                base_surface.blit(equip_text, (btn_rect.centerx - equip_text.get_width() // 2,
                                                btn_rect.centery - equip_text.get_height() // 2))

            y += 60
            if y > panel.bottom - 80:
                break  # Prevent overflow

    # Back button
    start_x = WIDTH//2 - 180
    start_y = HEIGHT - 120
    back_rect = pygame.Rect(start_x, start_y, 360, 48)

    is_hovered = back_rect.collidepoint(mouse_pos) or nav_is_focused("cosmetic_inventory", len(filtered_owned), len(filtered_owned) + 1)
    color = get_button_hover_color() if is_hovered else get_button_color()
    pygame.draw.rect(base_surface, color, back_rect, border_radius=12)
    pygame.draw.rect(base_surface, (180, 180, 180), back_rect, width=3 if is_hovered else 2, border_radius=12)

    back_text = menu_font.render(get_text("back"), True, get_text_color())
    base_surface.blit(back_text, (back_rect.centerx - back_text.get_width() // 2,
                            back_rect.centery - back_text.get_height() // 2))

    pygame.display.flip()
    return back_rect



def cosmetic_inventory_loop():
    global in_cosmetic_inventory, in_loadout_menu, in_profile_hub, cosmetics_state, cosmetic_tab, face_subcategory
    while in_cosmetic_inventory:
        back_rect = draw_cosmetic_inventory()

        # Registry der Item-Buttons (gleiche Iterationslogik wie draw_cosmetic_inventory)
        _ci_is_face_tab = COSMETIC_TAB_CATEGORIES[cosmetic_tab] == "face"
        _ci_panel_top = 186 if _ci_is_face_tab else 150
        _ci_panel = pygame.Rect(50, _ci_panel_top, WIDTH - 100, HEIGHT - _ci_panel_top - 60)
        _ci_y = _ci_panel_top + 10
        _ci_filtered, _ci_active = get_cosmetics_for_tab(cosmetic_tab)
        _ci_items = []
        for cosmetic in _ci_filtered:
            _ci_item_rect = pygame.Rect(_ci_panel.x + 20, _ci_y, _ci_panel.width - 40, 50)
            _ci_btn_rect = pygame.Rect(_ci_item_rect.right - 120, _ci_item_rect.y + 10, 100, 30)
            _ci_items.append((_ci_btn_rect, cosmetic))
            _ci_y += 60
            if _ci_y > _ci_panel.bottom - 80:
                break
        _ci_nav_rects = [r for r, _c in _ci_items] + [back_rect]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit()
                sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    _cosmetic_inventory_back()
                else:
                    d = nav_direction(event.key)
                    if d in ("left", "right"):
                        step = 1 if d == "right" else -1
                        cosmetic_tab = (cosmetic_tab + step) % len(COSMETIC_TAB_CATEGORIES)
                        face_subcategory = "eyes"  # Unterkategorie beim Haupt-Tab-Wechsel zurücksetzen
                        nav_set_focus("cosmetic_inventory", 0)
                    elif d in ("up", "down"):
                        cur = nav_get_focus("cosmetic_inventory", len(_ci_nav_rects))
                        nav_set_focus("cosmetic_inventory", nav_move(_ci_nav_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("cosmetic_inventory", len(_ci_nav_rects))
                        if _fi is not None and _fi < len(_ci_items):
                            cosmetic = _ci_items[_fi][1]
                            equip_toggle_cosmetic(cosmetic_tab, cosmetic, _ci_active)
                        elif _fi == len(_ci_items):
                            _cosmetic_inventory_back()
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                # Check tab clicks
                _tab_rects, _tab_w, _tab_h = get_cosmetic_tab_layout()
                _clicked_tab = False
                for i, trect in enumerate(_tab_rects):
                    if trect.collidepoint((mx, my)):
                        cosmetic_tab = i
                        _clicked_tab = True
                        break
                if _clicked_tab:
                    continue
                # Check Unterkategorie-Buttons (Augen/Nase/Mund), nur im "Gesichtsmerkmale"-Tab
                if COSMETIC_TAB_CATEGORIES[cosmetic_tab] == "face":
                    _clicked_sub = False
                    for i, srect in enumerate(get_face_subtab_layout()):
                        if srect.collidepoint((mx, my)):
                            face_subcategory = FACE_SUBCATEGORIES[i]
                            _clicked_sub = True
                            nav_set_focus("cosmetic_inventory", 0)
                            break
                    if _clicked_sub:
                        continue
                # Check back button
                if back_rect.collidepoint((mx, my)):
                    _cosmetic_inventory_back()
                    break
                # Check owned items and their equip/unequip buttons by reconstructing positions used in draw_cosmetic_inventory
                _is_face_tab = COSMETIC_TAB_CATEGORIES[cosmetic_tab] == "face"
                _panel_top = 186 if _is_face_tab else 150
                panel = pygame.Rect(50, _panel_top, WIDTH - 100, HEIGHT - _panel_top - 60)
                y = _panel_top + 10
                filtered_owned, active_equipped = get_cosmetics_for_tab(cosmetic_tab)
                for cosmetic in filtered_owned:
                    item_rect = pygame.Rect(panel.x + 20, y, panel.width - 40, 50)
                    # equip/unequip button area
                    btn_rect = pygame.Rect(item_rect.right - 120, item_rect.y + 10, 100, 30)
                    if btn_rect.collidepoint((mx, my)):
                        equip_toggle_cosmetic(cosmetic_tab, cosmetic, active_equipped)
                        break
                    y += 60
                    if y > panel.bottom - 80:
                        break
        clock.tick(60)



def draw_cosmetic_shop():
    global cosmetic_shop_scroll, _trail_anim_t, cosmetic_shop_tab, shop_face_subcategory
    _trail_anim_t = (_trail_anim_t + 0.06) % (2 * 3.14159265)
    mouse_pos = get_mouse_pos_logical()
    dark = is_dark_mode()
    ensure_daily_sale()  # Prüft (gecacht, siehe get_cached_current_date) ob heute ein neuer Sale nötig ist

    # Background
    fill_bg(rings=True)

    # Subtle decorative background circles
    deco_colors = [(200, 210, 255, 30), (220, 200, 255, 20)] if not dark else [(60, 80, 140, 40), (80, 50, 120, 30)]
    for i, dc in enumerate(deco_colors):
        s = pygame.Surface((300, 300), pygame.SRCALPHA)
        pygame.draw.circle(s, dc[:3] + (40,), (150, 150), 150)
        base_surface.blit(s, (i * 500 - 100, 50))

    # Title with shadow
    title_text = get_text("cosmetic_shop")
    title_shadow = font.render(title_text, True, (80, 80, 100))
    title_surf = font.render(title_text, True, get_text_color())
    base_surface.blit(title_shadow, (WIDTH // 2 - title_surf.get_width() // 2 + 2, 22))
    base_surface.blit(title_surf, (WIDTH // 2 - title_surf.get_width() // 2, 20))

    # Coin display top-right
    coin_bg = pygame.Rect(WIDTH - 175, 15, 160, 32)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=16)
    pygame.draw.rect(base_surface, (200, 180, 60), coin_bg, width=2, border_radius=16)
    pygame.draw.circle(base_surface, GOLD, (coin_bg.x + 18, coin_bg.centery), 8)
    pygame.draw.circle(base_surface, (180, 140, 0), (coin_bg.x + 18, coin_bg.centery), 8, width=2)
    coin_text = small_font.render(f"  {stats.get('coins', 0)} Coins", True, GOLD)
    base_surface.blit(coin_text, (coin_bg.x + 10, coin_bg.y + 7))

    # --- Kategorien-Tableiste (Spuren / Kopfbedeckungen / Gesichtsmerkmale / Tools) ---
    # Nutzt dieselbe Tab-Infrastruktur wie das Kosmetik-Inventar, aber mit eigenem
    # Tab-Zustand (cosmetic_shop_tab), damit Shop und Inventar unabhängig voneinander
    # auf einer Kategorie stehen bleiben können. Im Sale-Modus werden die Tabs nur
    # gedimmt angezeigt (nicht anklickbar aktiv), da die Sale-Ansicht kategorieübergreifend ist.
    # reserve_sale=True reserviert links davon den Platz für den Sale-Button.
    tab_rects, tab_w, tab_h = get_cosmetic_tab_layout(reserve_sale=True)
    tab_labels = [get_text(k) for k in COSMETIC_TAB_LABEL_KEYS]
    tab_label_font = tab_font if tab_w < 150 else small_font
    for i, (trect, tlabel) in enumerate(zip(tab_rects, tab_labels)):
        if (not cosmetic_shop_showing_sale) and cosmetic_shop_tab == i:
            tbg, tborder = get_button_color(), get_button_hover_color()
        else:
            tbg, tborder = get_panel_color(), (180, 180, 180)
        pygame.draw.rect(base_surface, tbg, trect, border_radius=12)
        pygame.draw.rect(base_surface, tborder, trect, width=2, border_radius=12)
        tsurf = tab_label_font.render(tlabel, True, get_text_color())
        base_surface.blit(tsurf, (trect.centerx - tsurf.get_width() // 2, trect.centery - tsurf.get_height() // 2))

    # --- Roter Sale-Button (links neben dem ersten Tab "Spuren") ---
    # Schaltet zwischen normaler Kategorie-Ansicht und einer Ansicht mit allen
    # heute reduzierten Items (über alle Kategorien hinweg) um. Text ist bewusst
    # NICHT übersetzt - soll in jeder Sprache schlicht "Sale" heißen.
    sale_rect = get_sale_button_rect()
    sale_hovered = sale_rect.collidepoint(mouse_pos)
    sale_bg = (230, 40, 40) if (cosmetic_shop_showing_sale or sale_hovered) else (170, 25, 25)
    pygame.draw.rect(base_surface, sale_bg, sale_rect, border_radius=15)
    pygame.draw.rect(base_surface, (255, 120, 120) if cosmetic_shop_showing_sale else (100, 15, 15), sale_rect, width=2, border_radius=15)
    sale_btn_text = small_font.render("Sale", True, (255, 255, 255))
    base_surface.blit(sale_btn_text, (sale_rect.centerx - sale_btn_text.get_width() // 2,
                                      sale_rect.centery - sale_btn_text.get_height() // 2))

    # Vertikaler Abstand zwischen Tab-Leiste (endet bei y=140) und dem Inhalt
    # darunter (Grid oder Sale-Titel), damit nichts mehr direkt aneinanderklebt.
    content_top = 140 + 20

    if cosmetic_shop_showing_sale:
        # Sale-Ansicht: keine Kategorie-/Unterkategorie-Filterung, zeigt alle
        # heute reduzierten Items zusammen an.
        resolved_category = "__sale__"
        # Titel + Countdown untereinander, sauber unter der Tab-Leiste, dann erst das Grid.
        sale_title_surf = menu_font.render(get_text("sale_view_title"), True, (255, 90, 90))
        base_surface.blit(sale_title_surf, (WIDTH // 2 - sale_title_surf.get_width() // 2, content_top))
        sale_countdown_surf = small_font.render(f"{get_text('sale_next_in')} {get_time_until_reset()}", True, get_text_color())
        base_surface.blit(sale_countdown_surf, (WIDTH // 2 - sale_countdown_surf.get_width() // 2,
                                                 content_top + sale_title_surf.get_height() + 4))
        grid_start_y = content_top + sale_title_surf.get_height() + 4 + sale_countdown_surf.get_height() + 16
    else:
        shop_category = COSMETIC_TAB_CATEGORIES[cosmetic_shop_tab]
        is_shop_face_tab = shop_category == "face"
        if is_shop_face_tab:
            sub_rects = get_face_subtab_layout()
            sub_labels = [get_text(k) for k in FACE_SUBCATEGORY_LABEL_KEYS]
            for i, (srect, slabel) in enumerate(zip(sub_rects, sub_labels)):
                active = FACE_SUBCATEGORIES[i] == shop_face_subcategory
                sbg = get_button_color() if active else get_panel_color()
                sborder = get_button_hover_color() if active else (150, 150, 150)
                pygame.draw.rect(base_surface, sbg, srect, border_radius=10)
                pygame.draw.rect(base_surface, sborder, srect, width=2, border_radius=10)
                stext = small_font.render(slabel, True, get_text_color())
                base_surface.blit(stext, (srect.centerx - stext.get_width() // 2, srect.centery - stext.get_height() // 2))
            resolved_category = shop_face_subcategory
            grid_start_y = sub_rects[0].bottom + 20
        else:
            resolved_category = shop_category
            grid_start_y = content_top

    # Grid: 3 Spalten, so viele Zeilen wie nötig. Nur Items der aktuell gewählten
    # Kategorie, sortiert nach Seltenheit, damit es nicht durcheinander wirkt; eine
    # unvollständige letzte Zeile wird zentriert statt links hängen zu bleiben.
    cols = 3
    tile_w, tile_h = 220, 155
    spacing_x, spacing_y = 18, 18
    _cosm_rarity_order = {"Common": 0, "Rare": 1, "Epic": 2, "Legendary": 3, "Mythic": 4}
    if resolved_category == "__sale__":
        items = sorted(
            ((k, COSMETICS_DB[k]) for k in get_sale_items() if k in COSMETICS_DB),
            key=lambda kv: _cosm_rarity_order.get(kv[1].get("rarity"), 9)
        )
    elif resolved_category is None:  # Tools-Tab: noch keine Items
        items = []
    else:
        items = sorted(
            ((k, m) for k, m in COSMETICS_DB.items() if m.get("category") == resolved_category),
            key=lambda kv: _cosm_rarity_order.get(kv[1].get("rarity"), 9)
        )
    total_grid_w = cols * tile_w + (cols - 1) * spacing_x
    grid_start_x = WIDTH // 2 - total_grid_w // 2

    item_rects = {}
    _owned_keys = set(cosmetics_state.get("owned", []))
    _total_buyable = sum(1 for key, _meta in items if key not in _owned_keys)
    _buy_idx = 0

    if resolved_category is None:
        msg = small_font.render(get_text("tools_coming_soon"), True, get_text_color())
        base_surface.blit(msg, (WIDTH // 2 - msg.get_width() // 2, grid_start_y + 60))
    elif resolved_category == "__sale__" and not items:
        msg = small_font.render(get_text("sale_no_items"), True, get_text_color())
        base_surface.blit(msg, (WIDTH // 2 - msg.get_width() // 2, grid_start_y + 60))

    for i, (key, meta) in enumerate(items):
        col = i % cols
        row = i // cols
        row_count = min(cols, len(items) - row * cols)
        row_x_offset = ((cols - row_count) * (tile_w + spacing_x)) // 2
        x = grid_start_x + row_x_offset + col * (tile_w + spacing_x)
        y = grid_start_y + row * (tile_h + spacing_y)
        item_rect = pygame.Rect(x, y, tile_w, tile_h)

        item_name = meta.get("name", {}).get(languages[language_index], key)
        price = get_cosmetic_price(key)
        base_price = get_cosmetic_base_price(key)
        discount = get_sale_discount(key)
        owned = key in cosmetics_state.get("owned", [])
        rarity = meta.get("rarity", "Common")
        rc = RARITY_COLORS.get(rarity, RARITY_COLORS["Common"])

        # Card shadow
        shadow_rect = pygame.Rect(x + 4, y + 4, tile_w, tile_h)
        pygame.draw.rect(base_surface, (0, 0, 0, 60) if dark else (180, 180, 200), shadow_rect, border_radius=14)

        # Card background (rarity-tinted, semi-dark in dark mode)
        if dark:
            card_bg = tuple(max(0, c - 40) for c in rc["bg"])
        else:
            card_bg = rc["bg"]
        pygame.draw.rect(base_surface, card_bg, item_rect, border_radius=14)

        # Hover glow border
        _cs_focused = (not owned) and nav_is_focused("cosmetic_shop", _buy_idx, _total_buyable + 1)
        if item_rect.collidepoint(mouse_pos) or _cs_focused:
            pygame.draw.rect(base_surface, rc["border"], item_rect, width=3, border_radius=14)
        else:
            pygame.draw.rect(base_surface, tuple(max(0, c - 50) for c in rc["border"]), item_rect, width=2, border_radius=14)

        # Rarity badge (top-left pill)
        badge_text = small_font.render(rc["label"], True, (255, 255, 255))
        badge_rect = pygame.Rect(x + 8, y + 8, badge_text.get_width() + 14, 20)
        pygame.draw.rect(base_surface, rc["badge"], badge_rect, border_radius=10)
        base_surface.blit(badge_text, (badge_rect.x + 7, badge_rect.y + 2))

        # Icon shape (top-right circle)
        icx, icy = x + tile_w - 28, y + 22
        pygame.draw.circle(base_surface, tuple(min(255, c + 40) for c in card_bg), (icx, icy), 18)
        pygame.draw.circle(base_surface, rc["border"], (icx, icy), 18, width=2)
        draw_cosmetic_icon(base_surface, key, icx, icy, big=False)

        # Item name
        name_surf = menu_font.render(item_name, True, (255, 255, 255))
        base_surface.blit(name_surf, (x + 10, y + 42))

        # Type label (trail / player)
        item_type = meta.get("type", "")
        type_surf = small_font.render(item_type.capitalize(), True, (200, 200, 220))
        base_surface.blit(type_surf, (x + 10, y + 68))

        # Divider line
        pygame.draw.line(base_surface, tuple(min(255, c + 30) for c in card_bg),
                         (x + 10, y + 88), (x + tile_w - 10, y + 88), 1)

        # Price with drawn coin. Bei aktivem Rabatt: durchgestrichener Originalpreis
        # links vom reduzierten (roten) Preis, damit die Ersparnis sofort sichtbar ist.
        pygame.draw.circle(base_surface, GOLD, (x + 20, y + 104), 7)
        pygame.draw.circle(base_surface, (200, 160, 0), (x + 20, y + 104), 7, width=2)
        if discount:
            base_price_surf = small_font.render(f"{base_price}", True, (170, 170, 170))
            base_surface.blit(base_price_surf, (x + 32, y + 96))
            strike_y = y + 96 + base_price_surf.get_height() // 2
            pygame.draw.line(base_surface, (170, 170, 170),
                              (x + 32, strike_y), (x + 32 + base_price_surf.get_width(), strike_y), 2)
            price_surf = small_font.render(f"{price}", True, (255, 70, 70))
            base_surface.blit(price_surf, (x + 32 + base_price_surf.get_width() + 8, y + 96))
        else:
            price_surf = small_font.render(f"{price}", True, GOLD)
            base_surface.blit(price_surf, (x + 32, y + 96))

        # Owned badge or Buy button
        if owned:
            owned_bg = pygame.Rect(x + tile_w - 90, y + tile_h - 34, 80, 24)
            pygame.draw.rect(base_surface, (30, 160, 60), owned_bg, border_radius=12)
            owned_surf = small_font.render(get_text("owned"), True, (255, 255, 255))
            base_surface.blit(owned_surf, (owned_bg.centerx - owned_surf.get_width() // 2,
                                           owned_bg.centery - owned_surf.get_height() // 2))
        else:
            buy_rect = pygame.Rect(x + tile_w - 90, y + tile_h - 34, 80, 24)
            is_hovered_buy = buy_rect.collidepoint(mouse_pos) or _cs_focused
            buy_bg = (255, 200, 20) if is_hovered_buy else (200, 160, 10)
            pygame.draw.rect(base_surface, buy_bg, buy_rect, border_radius=12)
            buy_text = small_font.render(get_text("buy"), True, (30, 20, 0))
            base_surface.blit(buy_text, (buy_rect.centerx - buy_text.get_width() // 2,
                                         buy_rect.centery - buy_text.get_height() // 2))
            item_rects[key] = buy_rect
            _buy_idx += 1

    # ---- Item Preview Panel ----
    _TRAIL_SWATCHES = {
        "fire_trail":    [(255, 245, 0), (255, 120, 0), (200, 40, 0)],
        "ice_trail":     [(220, 240, 255), (150, 200, 255), (80, 160, 255)],
        "shadow_trail":  [(120, 120, 120), (60, 60, 60), (20, 20, 20)],
        "sparkle_trail": [(255, 255, 230), (255, 230, 80), (200, 180, 0)],
        "toxic_trail":   [(150, 220, 100), (120, 180, 150), (100, 80, 180)],
        "rainbow_trail": [(230, 60, 60), (230, 220, 60), (70, 140, 230)],
        "star_trail":    [(255, 255, 255), (210, 230, 255), (140, 190, 255)],
        "smoke_trail":   [(220, 220, 225), (150, 150, 155), (80, 80, 85)],
        "blood_trail":   [(200, 20, 30), (140, 10, 20), (80, 5, 10)],
        "heart_trail":   [(255, 180, 200), (230, 40, 90), (160, 10, 50)],
        "yellow_crown":  None,  # Kein Trail — eigene Vorschau
    }

    hovered_key = None
    for idx, (key, meta) in enumerate(items):
        _c = idx % cols
        _r = idx // cols
        _row_count = min(cols, len(items) - _r * cols)
        _row_x_offset = ((cols - _row_count) * (tile_w + spacing_x)) // 2
        rx = grid_start_x + _row_x_offset + _c * (tile_w + spacing_x)
        ry = grid_start_y + _r * (tile_h + spacing_y)
        if pygame.Rect(rx, ry, tile_w, tile_h).collidepoint(mouse_pos):
            hovered_key = key
            break

    if hovered_key is None:
        _cs_fi = nav_get_focus("cosmetic_shop", _total_buyable + 1)
        if _cs_fi is not None and _cs_fi < _total_buyable:
            hovered_key = list(item_rects.keys())[_cs_fi]

    if hovered_key:
        pmeta = COSMETICS_DB[hovered_key]
        prarity = pmeta.get("rarity", "Common")
        prc = RARITY_COLORS.get(prarity, RARITY_COLORS["Common"])
        pname = pmeta.get("name", {}).get(languages[language_index], hovered_key)
        pprice = get_cosmetic_price(hovered_key)
        powned = hovered_key in cosmetics_state.get("owned", [])
        ptype = pmeta.get("type", "")
        pswatches = _TRAIL_SWATCHES.get(hovered_key, [(200, 200, 200)])

        # Panel-Bereich (schmaler Strip, unten zwischen Grid und Back-Button)
        # Dynamisch nach der letzten Grid-Zeile positioniert, statt fest auf 2 Zeilen
        # (COSMETICS_DB kann wachsen -> Grid hat dann mehr als 2 Zeilen)
        total_rows = (len(items) + cols - 1) // cols
        pv_w = cols * tile_w + (cols - 1) * spacing_x
        pv_h = 88
        pv_x = grid_start_x
        # Clamp gegen den Back-Button (HEIGHT - 52), falls das Grid mal zu viele Zeilen hat
        pv_y = min(grid_start_y + total_rows * (tile_h + spacing_y) + 6, HEIGHT - 52 - pv_h - 10)

        # Hintergrund
        pygame.draw.rect(base_surface, get_panel_color(), pygame.Rect(pv_x, pv_y, pv_w, pv_h), border_radius=12)
        pygame.draw.rect(base_surface, prc["border"], pygame.Rect(pv_x, pv_y, pv_w, pv_h), width=2, border_radius=12)

        # Grosses Icon links - für Kopfbedeckung/Augen/Nase/Mund wird der ECHTE Avatar
        # (eigene Farbe + aktuell ausgerüstetes Zeug + dieses Item probeweise in seinem
        # Slot, siehe cosmetics_overrides) gezeigt statt eines generischen Vektor-Icons,
        # damit die Vorschau exakt so aussieht wie im Spiel selbst. Trails bleiben beim
        # Icon, da ein bewegter Partikel-Trail auf einem statischen Bild keinen Sinn ergibt.
        icx2, icy2 = pv_x + 46, pv_y + pv_h // 2
        pslot = _COSMETIC_CATEGORY_TO_SLOT.get(pmeta.get("category"))
        if pslot:
            _preview_overrides = dict(cosmetics_state)
            _preview_overrides[pslot] = hovered_key
            _pv_avatar_rect = pygame.Rect(icx2 - 25, icy2 - 25, 50, 50)
            _pv_avatar_shadow = pygame.Rect(_pv_avatar_rect.x + 3, _pv_avatar_rect.y + 3, 50, 50)
            pygame.draw.rect(base_surface, (100, 100, 150), _pv_avatar_shadow, border_radius=8)
            draw_avatar(base_surface, _pv_avatar_rect, cosmetics_overrides=_preview_overrides)
        else:
            pygame.draw.circle(base_surface, tuple(min(255, ch + 30) for ch in prc["bg"]), (icx2, icy2), 32)
            pygame.draw.circle(base_surface, prc["border"], (icx2, icy2), 32, width=2)
            draw_cosmetic_icon(base_surface, hovered_key, icx2, icy2, big=True)

        # Name + Rarity + Typ
        tx = pv_x + 96
        name_s = menu_font.render(pname, True, get_text_color())
        base_surface.blit(name_s, (tx, pv_y + 8))

        # Rarity badge
        badge_s = small_font.render(prarity, True, (255, 255, 255))
        badge_bg = pygame.Rect(tx, pv_y + 36, badge_s.get_width() + 14, 20)
        pygame.draw.rect(base_surface, prc["badge"], badge_bg, border_radius=10)
        base_surface.blit(badge_s, (badge_bg.x + 7, badge_bg.y + 2))

        type_s = small_font.render(ptype.capitalize(), True, (180, 180, 200))
        base_surface.blit(type_s, (tx + badge_bg.width + 10, pv_y + 37))

        # Preis
        price_s = small_font.render(f"{pprice} Coins", True, GOLD)
        base_surface.blit(price_s, (tx, pv_y + 60))


        # Status-Text
        status_s = small_font.render("Besessen" if powned else "Nicht besessen", True,
                                     (80, 200, 120) if powned else (200, 120, 100))
        base_surface.blit(status_s, (pv_x + pv_w - status_s.get_width() - 14, pv_y + pv_h - 22))

    # Back button
    back_rect = pygame.Rect(WIDTH // 2 - 150, HEIGHT - 52, 300, 42)
    is_hovered_back = back_rect.collidepoint(mouse_pos) or nav_is_focused("cosmetic_shop", _total_buyable, _total_buyable + 1)
    back_bg = get_button_hover_color() if is_hovered_back else get_button_color()
    pygame.draw.rect(base_surface, back_bg, back_rect, border_radius=12)
    pygame.draw.rect(base_surface, (180, 180, 200), back_rect, width=2 if not is_hovered_back else 3, border_radius=12)
    back_text = menu_font.render(get_text("back"), True, get_text_color())
    base_surface.blit(back_text, (back_rect.centerx - back_text.get_width() // 2,
                                  back_rect.centery - back_text.get_height() // 2))

    # Overlays
    cleanup_overlays()
    for ov in overlays:
        txt = small_font.render(ov["text"], True, ov["color"])
        base_surface.blit(txt, ((WIDTH - txt.get_width()) // 2, 100))
    draw_achievement_popups(base_surface)

    pygame.display.flip()
    return back_rect, item_rects


def buy_fire_trail():
    if "fire_trail" in cosmetics_state.get("owned", []):
        return  # Already owned
    price = get_daily_price()
    if stats.get("coins", 0) < price:
        push_overlay(get_text("insufficient_coins"), RED)
        return
    stats["coins"] -= price
    cosmetics_state["owned"].append("fire_trail")
    save_section("stats", stats)
    save_section("cosmetics", cosmetics_state)
    push_overlay(get_text("item_acquired").format(get_text('fire_trail')), GOLD)

def cosmetic_shop_loop():
    global in_cosmetic_shop, in_shop_menu, cosmetic_shop_scroll, cosmetic_shop_tab, shop_face_subcategory, cosmetic_shop_showing_sale
    while in_cosmetic_shop:
        back_rect, buy_rects = draw_cosmetic_shop()
        _cs_keys  = list(buy_rects.keys())
        _cs_rects = list(buy_rects.values()) + [back_rect]
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit()
                sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    in_cosmetic_shop = False
                    in_shop_menu = True
                elif event.key == pygame.K_TAB:
                    # Tab-Taste zyklisch durch die Kosmetik-Kategorien (Spuren/Kopfbedeckungen/...)
                    cosmetic_shop_tab = (cosmetic_shop_tab + 1) % len(COSMETIC_TAB_CATEGORIES)
                    shop_face_subcategory = "eyes"
                    cosmetic_shop_showing_sale = False  # Kategorie-Wahl verlässt die Sale-Ansicht
                    nav_set_focus("cosmetic_shop", 0)
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("cosmetic_shop", len(_cs_rects))
                        nav_set_focus("cosmetic_shop", nav_move(_cs_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("cosmetic_shop", len(_cs_rects))
                        if _fi is not None and _fi < len(_cs_keys):
                            buy_cosmetic(_cs_keys[_fi])
                        elif _fi == len(_cs_keys):
                            in_cosmetic_shop = False
                            in_shop_menu = True
            if event.type == pygame.MOUSEBUTTONDOWN:
                mouse_pos = get_mouse_pos_logical()
                # Sale-Button anklicken: schaltet die kategorieübergreifende Angebots-
                # Ansicht um. Hat Vorrang vor den Kategorie-Tabs darunter.
                if get_sale_button_rect().collidepoint(mouse_pos):
                    cosmetic_shop_showing_sale = not cosmetic_shop_showing_sale
                    nav_set_focus("cosmetic_shop", 0)
                    play_sound(menu_click_sound)
                    continue
                # Kategorie-Tabs anklicken (reserve_sale=True: gleiche Positionen wie beim Zeichnen)
                _tab_rects, _tab_w, _tab_h = get_cosmetic_tab_layout(reserve_sale=True)
                _clicked_tab = False
                for i, trect in enumerate(_tab_rects):
                    if trect.collidepoint(mouse_pos):
                        cosmetic_shop_tab = i
                        shop_face_subcategory = "eyes"
                        cosmetic_shop_showing_sale = False  # Kategorie-Wahl verlässt die Sale-Ansicht
                        nav_set_focus("cosmetic_shop", 0)
                        _clicked_tab = True
                        break
                if _clicked_tab:
                    continue
                # Unterkategorie-Buttons (Augen/Nase/Mund), nur im "Gesichtsmerkmale"-Tab
                # und nur außerhalb der Sale-Ansicht (dort werden sie gar nicht gezeichnet)
                if (not cosmetic_shop_showing_sale) and COSMETIC_TAB_CATEGORIES[cosmetic_shop_tab] == "face":
                    _clicked_sub = False
                    for i, srect in enumerate(get_face_subtab_layout()):
                        if srect.collidepoint(mouse_pos):
                            shop_face_subcategory = FACE_SUBCATEGORIES[i]
                            nav_set_focus("cosmetic_shop", 0)
                            _clicked_sub = True
                            break
                    if _clicked_sub:
                        continue
                if back_rect and back_rect.collidepoint(mouse_pos):
                    in_cosmetic_shop = False
                    in_shop_menu = True
                elif buy_rects:
                    for cosmetic_key, rect in buy_rects.items():
                        if rect.collidepoint(mouse_pos):
                            buy_cosmetic(cosmetic_key)
                            break
        clock.tick(60)




def to_power_roll():
    global in_shop_menu, in_shop
    in_shop_menu = False
    in_shop = True
    play_sound(menu_click_sound)


def to_cosmetic_shop():
    global in_shop_menu, in_cosmetic_shop
    in_shop_menu = False
    in_cosmetic_shop = True
    play_sound(menu_click_sound)




def draw_shop_menu():
    mouse_pos = get_mouse_pos_logical()
    fill_bg(rings=True)

    title = font.render(get_text("shop"), True, get_text_color())
    base_surface.blit(title, (WIDTH // 2 - title.get_width() // 2, 70))
    
    # Coin-Anzeige
    coin_bg = pygame.Rect(15, 15, 200, 35)
    pygame.draw.rect(base_surface, get_panel_color(), coin_bg, border_radius=18)
    pygame.draw.rect(base_surface, (200, 200, 200), coin_bg, width=2, border_radius=18)
    
    coin_text = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(coin_text, (25, 22))
    
    # Buttons
    start_x = WIDTH//2 - 180
    start_y = 180
    gap = 70
    power_roll_rect = pygame.Rect(start_x, start_y + 0*gap, 360, 48)
    cosmetic_shop_rect = pygame.Rect(start_x, start_y + 1*gap, 360, 48)
    back_rect = pygame.Rect(start_x, start_y + 2*gap, 360, 48)
    
    _sm_labels = [(power_roll_rect, "power_roll"), (cosmetic_shop_rect, "cosmetic_shop"), (back_rect, "back")]
    for i, (rect, text_key) in enumerate(_sm_labels):
        is_hovered = rect.collidepoint(mouse_pos) or nav_is_focused("shop_menu", i, len(_sm_labels))
        color = get_button_hover_color() if is_hovered else get_button_color()
        pygame.draw.rect(base_surface, color, rect, border_radius=12)
        pygame.draw.rect(base_surface, (180, 180, 180), rect, width=3 if is_hovered else 2, border_radius=12)

        text_surf = menu_font.render(get_text(text_key), True, get_text_color())
        base_surface.blit(text_surf, (rect.centerx - text_surf.get_width() // 2,
                                rect.centery - text_surf.get_height() // 2))

    pygame.display.flip()


def shop_menu_loop():
    global in_shop_menu, in_menu
    while in_shop_menu:
        start_x = WIDTH//2 - 180
        start_y = 180
        gap = 70
        power_roll_rect = pygame.Rect(start_x, start_y + 0*gap, 360, 48)
        cosmetic_shop_rect = pygame.Rect(start_x, start_y + 1*gap, 360, 48)
        back_rect = pygame.Rect(start_x, start_y + 2*gap, 360, 48)
        _sm_rects = [power_roll_rect, cosmetic_shop_rect, back_rect]
        _sm_actions = ["power_roll", "cosmetic_shop", "back"]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit()
                sys.exit()
            _sm_action = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    _sm_action = "back"
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("shop_menu", len(_sm_rects))
                        nav_set_focus("shop_menu", nav_move(_sm_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _sm_action = _sm_actions[nav_get_focus("shop_menu", len(_sm_rects))]
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mouse_pos = get_mouse_pos_logical()
                mx, my = mouse_pos
                if power_roll_rect.collidepoint((mx, my)):
                    _sm_action = "power_roll"
                elif cosmetic_shop_rect.collidepoint((mx, my)):
                    _sm_action = "cosmetic_shop"
                elif back_rect.collidepoint((mx, my)):
                    _sm_action = "back"
            if _sm_action == "power_roll":
                play_sound(menu_click_sound)
                to_power_roll()
            elif _sm_action == "cosmetic_shop":
                play_sound(menu_click_sound)
                to_cosmetic_shop()
            elif _sm_action == "back":
                play_sound(click_back_sound)
                in_shop_menu = False
                in_menu = True
        draw_shop_menu()
        clock.tick(60)




# =============================
# Loadout / Inventory (Tabs: Owned / Equipped) + Auto-swap & highlight
# =============================

loadout_item_rects = []
loadout_tab = 0  # 0 = owned, 1 = all
loadout_scroll = 0
cosmetic_tab = 0  # 0 = trails, 1 = player
cosmetic_shop_scroll = 0.0
cosmetic_shop_tab = 0  # aktueller Kategorie-Tab im Kosmetik-Shop (unabhängig vom Inventar-Tab)
shop_face_subcategory = "eyes"  # aktuelle Unterkategorie (Augen/Nase/Mund) im Shop-"Gesichtsmerkmale"-Tab
cosmetic_shop_showing_sale = False  # True = Sale-Ansicht (reduzierte Items aller Kategorien) statt normalem Tab

def draw_hover_tooltip(content, mouse_pos, max_width=280, accent_color=None, anchor_rect=None):
    """Zeichnet eine kleine Erklärungs-Box in der Nähe des Cursors (an den Bildschirmrand
    geklemmt, damit sie nie abgeschnitten wird) - die einheitliche Hover-Erklärung für's
    ganze Spiel (Powers, Schwierigkeiten, Custom-Modifier, Cosmetics, ...).

    `content` ist entweder ein einzelner String (wird automatisch wortweise umgebrochen)
    oder eine Liste/ein Tupel bereits fertiger Zeilen (wird 1:1 dargestellt, zu lange
    Zeilen werden verkleinert statt neu umgebrochen - z.B. für die zweizeiligen
    Schwierigkeits-Erklärungen). `accent_color` färbt optional den Rahmen ein.

    `anchor_rect`: wenn gesetzt (Tastatur-Fokus statt Maus-Hover), wird die Box neben
    diesem Rect statt neben `mouse_pos` platziert."""
    if not content:
        return
    text_color = (240, 240, 245)
    if isinstance(content, str):
        words = content.split(' ')
        raw_lines = []
        line = ''
        for w in words:
            test = (line + ' ' + w).strip()
            if line and tooltip_font.size(test)[0] > max_width - 16:
                raw_lines.append(line)
                line = w
            else:
                line = test
        if line:
            raw_lines.append(line)
    else:
        raw_lines = list(content)

    line_surfs = []
    for ln in raw_lines:
        ls = tooltip_font.render(ln, True, text_color)
        if ls.get_width() > max_width - 16:
            ls = _fit_surf(ls, max_width - 16)
        line_surfs.append(ls)
    if not line_surfs:
        return

    line_h = tooltip_font.get_height() + 2
    box_w = min(max_width, max(ls.get_width() for ls in line_surfs) + 16)
    box_h = len(line_surfs) * line_h + 12
    if anchor_rect is not None:
        x = anchor_rect.right + 12
        y = anchor_rect.centery - box_h // 2
        if x + box_w > WIDTH:
            x = anchor_rect.left - box_w - 12
    else:
        x = mouse_pos[0] + 16
        y = mouse_pos[1] + 16
        if x + box_w > WIDTH:
            x = mouse_pos[0] - box_w - 16
        if y + box_h > HEIGHT:
            y = mouse_pos[1] - box_h - 16
    x = max(4, min(x, WIDTH - box_w - 4))
    y = max(4, min(y, HEIGHT - box_h - 4))

    tip_surf = pygame.Surface((box_w, box_h), pygame.SRCALPHA)
    pygame.draw.rect(tip_surf, (25, 25, 35, 235), tip_surf.get_rect(), border_radius=8)
    border_col = (*accent_color, 255) if accent_color else (150, 180, 220, 255)
    pygame.draw.rect(tip_surf, border_col, tip_surf.get_rect(), width=2 if accent_color else 1, border_radius=8)
    for i, ls in enumerate(line_surfs):
        tip_surf.blit(ls, (8, 6 + i * line_h))
    base_surface.blit(tip_surf, (x, y))


def draw_loadout():
    global loadout_item_rects, loadout_tab, loadout_overlay
    fill_bg(rings=True)
    mouse_pos = get_mouse_pos_logical()
    title = font.render(get_text("loadout"), True, get_text_color())
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 20))

    slots = get_max_slots()
    slot_txt = small_font.render(f"Slots: {len(powers_state['equipped'])}/{slots}", True, get_text_color())
    base_surface.blit(slot_txt, (20, 20))

    # Tabs at top (Owned / All) - der frühere "Ausgerüstet"-Tab wurde entfernt: er zeigte nur
    # eine Teilmenge, die (mit dem ⚡-Indikator) schon in Owned/All sichtbar ist, und sein
    # einziger Klick-Effekt war ein Neuordnen statt eines echten Unequip.
    tab_w = 150
    tab_h = 40
    x_start = WIDTH//2 - (2 * tab_w + 8) // 2
    y_tabs = 80

    owned_label = get_text("owned_tab")
    all_label = get_text("all_tab")

    # Tab-Rectangles
    owned_rect = pygame.Rect(x_start, y_tabs, tab_w, tab_h)
    all_rect = pygame.Rect(x_start + tab_w + 8, y_tabs, tab_w, tab_h)

    # Tabs zeichnen mit Hover-Highlight
    for i, (rect, label) in enumerate([(owned_rect, owned_label), (all_rect, all_label)]):
        active = loadout_tab == i
        hovered = rect.collidepoint(mouse_pos)
        if active:
            bg_color, border_color = get_button_color(), get_button_hover_color()
        else:
            bg_color = get_button_hover_color() if hovered else get_panel_color()
            border_color = (150, 180, 220) if hovered else (180, 180, 180)
        pygame.draw.rect(base_surface, bg_color, rect, border_radius=12)
        pygame.draw.rect(base_surface, border_color, rect, width=2, border_radius=12)
        text_surf = small_font.render(label, True, get_text_color())
        base_surface.blit(text_surf, (rect.centerx - text_surf.get_width() // 2,
                                rect.centery - text_surf.get_height() // 2))

        # Kleiner Indikator für Anzahl
        count = len(powers_state.get("owned", [])) if i == 0 else len(POWERS_LIST)
        count_text = small_font.render(f"({count})", True, get_text_color())
        base_surface.blit(count_text, (rect.right - count_text.get_width() - 8, rect.y + 4))

    info = small_font.render(get_text("click_equip"), True, get_text_color())
    base_surface.blit(info, (WIDTH//2 - info.get_width()//2, HEIGHT-40))

    y = 140 - loadout_scroll
    loadout_item_rects = []
    # Choose items based on tab: 0 = Owned, 1 = All
    if loadout_tab == 0:
        items = powers_state.get("owned", [])
    else:
        # Nach Seltenheit sortiert (aufsteigend) statt POWERS_DB-Definitionsreihenfolge -
        # sonst landet ein später ergänztes Power (z.B. Magnet) einfach hinten, unabhängig
        # von seiner Seltenheit, wie es vor diesem Fix der Fall war.
        _power_rarity_order = {"Common": 0, "Rare": 1, "Epic": 2, "Legendary": 3, "Mythic": 4}
        items = sorted(POWERS_LIST, key=lambda n: _power_rarity_order.get(POWERS_DB.get(n, {}).get("rarity"), 9))

    if not items:
        msg = small_font.render(get_text("no_owned"), True, DARK_GRAY)
        base_surface.blit(msg, (100, y))

    hovered_name = None
    hovered_name_rect = None
    for idx, name in enumerate(items):
        rar = POWERS_DB.get(name, {}).get("rarity", "?")
        color = rarity_colors.get(rar, GRAY)
        rect = pygame.Rect(100, y, 600, 40)
        _row_mouse_hover = rect.collidepoint(mouse_pos)
        row_hovered = _row_mouse_hover or nav_is_focused("loadout", idx, len(items))
        if row_hovered:
            hovered_name = name
            hovered_name_rect = None if _row_mouse_hover else rect

        # Verbessertes Design für Power-Einträge
        owned = name in powers_state.get("owned", [])
        equipped = name in powers_state.get("equipped", [])

        # Hintergrund basierend auf Status
        if loadout_tab == 1 and not owned:  # Locked in All tab
            bg_color = get_panel_color()
            border_color = (200, 200, 200)
        elif equipped:  # Equipped (wird in Owned wie All angezeigt)
            bg_color = get_button_hover_color()
            border_color = (100, 200, 100)
        else:  # Normal
            bg_color = get_panel_color()
            border_color = (200, 200, 200)
        if row_hovered:
            border_color = (150, 180, 220)

        # Hintergrund zeichnen
        pygame.draw.rect(base_surface, bg_color, rect, border_radius=8)
        pygame.draw.rect(base_surface, border_color, rect, width=2 if row_hovered else 1, border_radius=8)

        # Seltenheits-Indikator (größer und schöner)
        rarity_rect = pygame.Rect(105, y + 5, 30, 30)
        pygame.draw.rect(base_surface, color, rarity_rect, border_radius=6)
        pygame.draw.rect(base_surface, (0, 0, 0), rarity_rect, width=1, border_radius=6)

        # Power-Name und Status
        translated_name = get_power_name(name)
        label = translated_name
        if loadout_tab == 1 and not owned:
            label += f" - {get_text('locked')}"
        elif equipped:
            label += f" - {get_text('equipped')}"

        # Name
        name_text = small_font.render(label, True, get_text_color())
        base_surface.blit(name_text, (145, y + 8))

        # Seltenheit
        rarity_text = small_font.render(f"[{rar}]", True, get_text_color())
        base_surface.blit(rarity_text, (145, y + 22))

        # Beschreibung (falls verfügbar) - im All-Tab direkt in der Zeile sichtbar,
        # im Owned-Tab reicht der Platz nicht, dafür gibt's die Hover-Tooltip weiter unten
        desc = get_power_desc(name)
        if desc and loadout_tab == 1:
            desc_text = small_font.render(desc, True, get_text_color())
            base_surface.blit(desc_text, (300, y + 15))

        loadout_item_rects.append((rect, name))
        y += 45

    # Hover-Tooltip mit der Kurzbeschreibung - v.a. im Owned-Tab nützlich (wo die
    # Beschreibung nicht direkt in der Zeile steht); im All-Tab steht sie schon in der
    # Zeile, dort würde der Tooltip sie nur redundant verdoppeln.
    if hovered_name and not (loadout_tab == 1 and get_power_desc(hovered_name)):
        draw_hover_tooltip(get_power_desc(hovered_name), mouse_pos, anchor_rect=hovered_name_rect)

    # If an overlay (power detail) is active, draw it
    if 'loadout_overlay' in globals() and loadout_overlay:
        name = loadout_overlay.get('name')
        desc = get_power_desc(name, detailed=True)
        rar = POWERS_DB.get(name,{}).get('rarity','?')
        
        # Backdrop
        s = pygame.Surface((WIDTH, HEIGHT))
        s.set_alpha(200)
        s.fill(OVERLAY_BG)
        base_surface.blit(s, (0,0))

        # Panel - größer für mehr Details
        panel = pygame.Rect(60, 60, WIDTH-120, HEIGHT-120)
        pygame.draw.rect(base_surface, get_panel_color(), panel, border_radius=16)
        pygame.draw.rect(base_surface, (60,80,140), panel, width=3, border_radius=16)

        # Titel mit Seltenheitsfarbe
        rarity_color = rarity_colors.get(rar, GRAY)
        title = menu_font.render(f"{get_power_name(name)}", True, get_text_color())
        rarity_text = menu_font.render(f"[{rar}]", True, rarity_color)
        base_surface.blit(title, (panel.x + 20, panel.y + 20))
        base_surface.blit(rarity_text, (panel.x + 20 + title.get_width() + 10, panel.y + 20))

        # Seltenheits-Indikator
        rarity_rect = pygame.Rect(panel.x + 20, panel.y + 50, 30, 30)
        pygame.draw.rect(base_surface, rarity_color, rarity_rect, border_radius=8)
        pygame.draw.rect(base_surface, (0, 0, 0), rarity_rect, width=2, border_radius=8)

        # Beschreibung
        desc_label = small_font.render(get_text("description") + ":", True, get_text_color())
        base_surface.blit(desc_label, (panel.x + 60, panel.y + 55))

        # Description (wrap besser)
        lines = []
        line = ''
        max_chars = 70  # Mehr Zeichen pro Zeile
        for word in desc.split(' '):
            if len(line + ' ' + word) > max_chars:
                if line:
                    lines.append(line)
                line = word
            else:
                line = (line + ' ' + word).strip()
        if line:
            lines.append(line)

        y0 = panel.y + 85
        for ln in lines:
            lsurf = small_font.render(ln, True, get_text_color())
            base_surface.blit(lsurf, (panel.x + 20, y0))
            y0 += 24

        # Zusätzliche Power-Informationen
        y0 += 10
        power_data = POWERS_DB.get(name, {})

        # Cooldown Info
        if "cd" in power_data:
            cd_text = small_font.render(f"Cooldown: {power_data['cd']}s", True, get_text_color())
            base_surface.blit(cd_text, (panel.x + 20, y0))
            y0 += 24

        # Duration Info
        if "duration" in power_data:
            duration = power_data["duration"]
            if isinstance(duration, tuple):
                duration_text = small_font.render(f"Duration: {duration[0]}-{duration[1]}s", True, get_text_color())
            else:
                duration_text = small_font.render(f"Duration: {duration}s", True, get_text_color())
            base_surface.blit(duration_text, (panel.x + 20, y0))
            y0 += 24

        # Besitz-Status
        owned = name in powers_state.get("owned", [])
        equipped = name in powers_state.get("equipped", [])

        if owned:
            status_text = small_font.render("✅ " + get_text("owned_tab"), True, (0, 150, 0))
        else:
            status_text = small_font.render("❌ " + get_text("not_owned"), True, (150, 0, 0))
        base_surface.blit(status_text, (panel.x + 20, y0))
        y0 += 24

        if equipped:
            equip_text = small_font.render(get_text("equipped_tab"), True, (0, 100, 200))
            base_surface.blit(equip_text, (panel.x + 20, y0))
            y0 += 24

        # Schließen-Hinweis
        hint = small_font.render(get_text("enter_click_esc_close_hint"), True, get_text_color())
        base_surface.blit(hint, (panel.centerx - hint.get_width()//2, panel.bottom - 40))

    pygame.display.flip()

def loadout_loop():
    global in_loadout, in_menu, loadout_tab, loadout_overlay
    loadout_overlay = None
    while in_loadout:
        draw_loadout()
        _lo_rects = [r for r, _n in loadout_item_rects]
        _lo_fade_top, _lo_fade_bottom = 140, HEIGHT - 50
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    # Close overlay first if open
                    if loadout_overlay:
                        loadout_overlay = None
                    else:
                        in_loadout = False
                        in_menu = True
                elif event.key == pygame.K_TAB:
                    loadout_tab = (loadout_tab + 1) % 2
                elif loadout_overlay:
                    if is_nav_confirm(event.key):
                        # Close overlay on ENTER/SPACE
                        loadout_overlay = None
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("loadout", len(_lo_rects))
                        new_i = nav_move(_lo_rects, cur, d)
                        if new_i != cur:
                            nav_set_focus("loadout", new_i)
                            adjust_loadout_scroll(nav_reveal_delta(_lo_rects[new_i], _lo_fade_top, _lo_fade_bottom))
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("loadout", len(_lo_rects))
                        if _fi is not None and _fi < len(loadout_item_rects):
                            name = loadout_item_rects[_fi][1]
                            play_sound(click_sound)
                            if loadout_tab == 1:
                                desc = get_power_desc(name, detailed=True)
                                rar = POWERS_DB.get(name, {}).get('rarity', '?')
                                loadout_overlay = {'name': name, 'desc': desc, 'rarity': rar}
                            elif name == "Extra Slot":
                                push_overlay("Extra Slot ist passiv und kann nicht ausgerüstet werden", (180, 80, 80))
                            elif name in powers_state["equipped"]:
                                push_overlay(f"{name} bereits ausgerüstet", (120,120,120))
                            else:
                                if len(powers_state["equipped"]) < get_max_slots():
                                    powers_state["equipped"].append(name)
                                    push_overlay(f"Equipped: {get_power_name(name)}", (80,160,80))
                                else:
                                    if powers_state["equipped"]:
                                        dropped = powers_state["equipped"].pop(0)
                                        powers_state["equipped"].append(name)
                                        push_overlay(f"Auto-Swap: {get_power_name(dropped)} → {get_power_name(name)}", (180,140,20))
                                    else:
                                        powers_state["equipped"].append(name)
                                        push_overlay(f"Equipped: {get_power_name(name)}", (80,160,80))
                                save_section("powers", powers_state)
            elif event.type == pygame.MOUSEWHEEL:
                adjust_loadout_scroll(-event.y * 30)
            if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:  # Only left click
                mx, my = get_mouse_pos_logical()
                # Tab clicks - Owned / All
                tab_w = 150
                tab_h = 40
                x_start = WIDTH//2 - (2 * tab_w + 8) // 2
                y_tabs = 80
                owned_rect = pygame.Rect(x_start, y_tabs, tab_w, tab_h)
                all_rect = pygame.Rect(x_start + tab_w + 8, y_tabs, tab_w, tab_h)
                if owned_rect.collidepoint((mx,my)):
                    play_sound(click_sound, "click_volume")
                    loadout_tab = 0
                    loadout_overlay = None
                    continue
                if all_rect.collidepoint((mx,my)):
                    play_sound(click_sound)
                    loadout_tab = 1
                    loadout_overlay = None
                    continue

                # If overlay open: clicking anywhere closes it (or clicking outside panel)
                if loadout_overlay:
                    loadout_overlay = None
                    continue

                for rect, name in loadout_item_rects:
                    if rect.collidepoint((mx,my)):
                        play_sound(click_sound)
                        # If in All tab: show details popup
                        if loadout_tab == 1:
                            desc = get_power_desc(name, detailed=True)
                            rar = POWERS_DB.get(name, {}).get('rarity', '?')
                            loadout_overlay = {'name': name, 'desc': desc, 'rarity': rar}
                            break
                        # Owned tab: equip (with auto-swap if slots are full)
                        if name == "Extra Slot":
                            push_overlay("Extra Slot ist passiv und kann nicht ausgerüstet werden", (180, 80, 80))
                            continue
                        if name in powers_state["equipped"]:
                            push_overlay(f"{name} bereits ausgerüstet", (120,120,120))
                        else:
                            if len(powers_state["equipped"]) < get_max_slots():
                                powers_state["equipped"].append(name)
                                push_overlay(f"Equipped: {get_power_name(name)}", (80,160,80))
                            else:
                                # Auto-swap oldest
                                if powers_state["equipped"]:
                                    dropped = powers_state["equipped"].pop(0)
                                    powers_state["equipped"].append(name)
                                    push_overlay(f"Auto-Swap: {get_power_name(dropped)} → {get_power_name(name)}", (180,140,20))
                                else:
                                    powers_state["equipped"].append(name)
                                    push_overlay(f"Equipped: {get_power_name(name)}", (80,160,80))
                        save_section("powers", powers_state)
                        break


# =============================
# Admin: Login & Panel
# =============================

in_admin_login = False
in_admin_panel = False
in_achievements_screen = False
admin_pwd_input = ""
admin_msg = ""
admin_amount_input = "0"


def draw_admin_login():
    fill_bg(rings=True)
    title = font.render(get_text("admin"), True, get_text_color())
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 40))

    panel = pygame.Rect(70, 110, 460, 220)
    draw_card(base_surface, panel, radius=14)

    label = small_font.render(get_text("enter_password"), True, get_text_color())
    base_surface.blit(label, (100, 150))

    # Input box (masked)
    box = pygame.Rect(100, 182, 420, 40)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=8)
    pygame.draw.rect(base_surface, (90, 130, 210), box, width=2, border_radius=8)
    masked = "*" * len(admin_pwd_input)
    txt = small_font.render(masked, True, get_text_color())
    base_surface.blit(txt, (box.x + 10, box.y + 8))

    hint = small_font.render(get_text("enter_login_menu_hint"), True, get_text_color())
    base_surface.blit(hint, (100, 240))

    if admin_msg:
        msg = small_font.render(admin_msg, True, (220, 60, 60))
        base_surface.blit(msg, (100, 280))

    pygame.display.flip()


def admin_login_loop():
    global in_admin_login, in_admin_panel, in_menu, admin_pwd_input, admin_msg
    while in_admin_login:
        draw_admin_login()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key in (pygame.K_ESCAPE, pygame.K_m):
                    in_admin_login = False
                    in_menu = True
                elif event.key == pygame.K_BACKSPACE:
                    admin_pwd_input = admin_pwd_input[:-1]
                elif event.key == pygame.K_RETURN:
                    if admin_pwd_input == ADMIN_PASSWORD:
                        admin_pwd_input = ""
                        admin_msg = ""
                        in_admin_login = False
                        in_admin_panel = True
                    else:
                        admin_msg = get_text("wrong_password")
                else:
                    # accept simple printable chars
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(admin_pwd_input) < 64:
                        admin_pwd_input += ch


def draw_admin_panel():
    global admin_power_rects
    fill_bg(rings=True)
    title = font.render(get_text("admin"), True, get_text_color())
    base_surface.blit(title, (WIDTH//2 - title.get_width()//2, 40))

    panel = pygame.Rect(60, 95, WIDTH - 120, HEIGHT - 110)
    draw_card(base_surface, panel, radius=16)

    # Current coins
    cur = small_font.render(f"{get_text('coins')}: {stats.get('coins',0)}", True, get_text_color())
    base_surface.blit(cur, (100, 120))

    # Amount input
    label = small_font.render(get_text("amount") + ":", True, get_text_color())
    base_surface.blit(label, (100, 164))

    box = pygame.Rect(200, 156, 140, 40)
    pygame.draw.rect(base_surface, get_bg_color(), box, border_radius=8)
    pygame.draw.rect(base_surface, (90, 130, 210), box, width=2, border_radius=8)
    txt = small_font.render(admin_amount_input, True, get_text_color())
    base_surface.blit(txt, (box.x + 10, box.y + 8))

    # Buttons
    add_btn = pygame.Rect(360, 156, 200, 40)
    set_btn = pygame.Rect(360, 206, 200, 40)
    logout_btn = pygame.Rect(WIDTH - 160, HEIGHT - 60, 140, 40)

    mp = get_mouse_pos_logical()
    for rect, label_key in [(add_btn, "add_coins"), (set_btn, "set_coins"), (logout_btn, "logout")]:
        draw_ui_button(base_surface, rect, get_text(label_key), font=small_font, hovered=rect.collidepoint(mp))

    # Power buttons
    power_label = small_font.render(get_text("give_power") + ":", True, get_text_color())
    base_surface.blit(power_label, (100, 260))

    admin_power_rects = []
    y = 290
    for i, power_name in enumerate(POWERS_LIST[:8]):  # Show first 8 powers
        if i % 2 == 0 and i > 0:
            y += 35
        x = 100 + (i % 2) * 300
        rect = pygame.Rect(x, y, 280, 30)
        admin_power_rects.append((rect, power_name))

        # Color by rarity
        rar = POWERS_DB.get(power_name, {}).get("rarity", "?")
        color = rarity_colors.get(rar, GRAY)
        hov = rect.collidepoint(mp)
        pygame.draw.rect(base_surface, color, rect, border_radius=6)
        pygame.draw.rect(base_surface, (255, 255, 255) if hov else DARK_GRAY, rect, width=(2 if hov else 1), border_radius=6)

        # Power name
        txt = small_font.render(get_power_name(power_name), True, get_text_color())
        base_surface.blit(txt, (rect.x + 8, rect.y + 6))

    hint = small_font.render(get_text("enter_confirm_menu_hint"), True, get_text_color())
    base_surface.blit(hint, (100, HEIGHT-40))

    pygame.display.flip()
    return add_btn, set_btn, logout_btn, box


def admin_panel_loop():
    global in_admin_panel, in_menu, admin_amount_input
    while in_admin_panel:
        add_btn, set_btn, logout_btn, amount_box = draw_admin_panel()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key in (pygame.K_ESCAPE, pygame.K_m):
                    in_admin_panel = False
                    in_menu = True
                elif event.key == pygame.K_BACKSPACE:
                    admin_amount_input = admin_amount_input[:-1] if admin_amount_input else ""
                elif event.key == pygame.K_RETURN:
                    # default ENTER → add
                    apply_admin_add()
                else:
                    ch = event.unicode
                    if ch and ch.isdigit() and len(admin_amount_input) < 9:
                        admin_amount_input += ch
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                if logout_btn.collidepoint((mx,my)):
                    play_sound(click_back_sound)
                    in_admin_panel = False
                    in_menu = True
                elif add_btn.collidepoint((mx,my)):
                    play_sound(click_sound)
                    apply_admin_add()
                elif set_btn.collidepoint((mx,my)):
                    play_sound(click_sound)
                    apply_admin_set()
                else:
                    for rect, name in admin_power_rects:
                        if rect.collidepoint((mx, my)):
                            play_sound(click_sound)
                            apply_admin_give_power(name)
                            break



def apply_admin_add():
    try:
        n = int(admin_amount_input or "0")
        if n <= 0: return
        stats["coins"] = stats.get("coins", 0) + n
        save_section("stats", stats)
    except Exception:
        pass


def apply_admin_set():
    try:
        n = int(admin_amount_input or "0")
        if n < 0: n = 0
        stats["coins"] = n
        save_section("stats", stats)
    except Exception:
        pass


def apply_admin_give_power(power_name):
    """Gibt dem Spieler eine Power über das Admin-Panel"""
    if power_name not in powers_state["owned"]:
        powers_state["owned"].append(power_name)
        save_section("powers", powers_state)
        push_overlay(f"Power '{get_power_name(power_name)}' hinzugefügt!", GOLD)
    else:
        push_overlay(f"Power '{get_power_name(power_name)}' bereits besessen", (200, 200, 0))


# =============================
# Mod System (inline)
# =============================

import importlib.util as _ilu

class _BeginnerAPI:
    def __init__(self, g): self._g = g

    # ── Player ─────────────────────────────────────────────
    @property
    def player_hp(self): return self._g.get('lives', 0)
    @player_hp.setter
    def player_hp(self, v): self._g['lives'] = max(0, int(v))

    @property
    def player_x(self): return self._g.get('player_x', 0)
    @player_x.setter
    def player_x(self, v): self._g['player_x'] = int(v)

    @property
    def player_y(self): return self._g.get('player_y', 0)
    @player_y.setter
    def player_y(self, v): self._g['player_y'] = int(v)

    @property
    def player_vy(self): return self._g.get('player_vy', 0)
    @player_vy.setter
    def player_vy(self, v): self._g['player_vy'] = float(v)

    @property
    def is_jumping(self): return self._g.get('is_jumping', False)
    @is_jumping.setter
    def is_jumping(self, v): self._g['is_jumping'] = bool(v)

    @property
    def used_double_jump(self): return self._g.get('used_double_jump', False)
    @used_double_jump.setter
    def used_double_jump(self, v): self._g['used_double_jump'] = bool(v)

    # ── Score / inventory ───────────────────────────────────
    @property
    def score(self): return self._g.get('score', 0)
    @score.setter
    def score(self, v): self._g['score'] = max(0, int(v))

    @property
    def inventory(self): return self._g.get('inventory', [])

    # ── Physics ─────────────────────────────────────────────
    @property
    def move_speed(self): return self._g.get('move_speed', 8)
    @move_speed.setter
    def move_speed(self, v): self._g['move_speed'] = max(0, float(v))

    @property
    def gravity(self): return self._g.get('gravity', 1)
    @gravity.setter
    def gravity(self, v): self._g['gravity'] = float(v)

    # ── Screen ──────────────────────────────────────────────
    @property
    def width(self): return self._g.get('width', 600)
    @property
    def height(self): return self._g.get('height', 400)

    # ── Obstacles / spawning ────────────────────────────────
    @property
    def obstacle_timer(self): return self._g.get('obstacle_timer', 0)
    @obstacle_timer.setter
    def obstacle_timer(self, v): self._g['obstacle_timer'] = int(v)

    @property
    def obstacle_delay(self): return self._g.get('obstacle_delay', 1000)
    @obstacle_delay.setter
    def obstacle_delay(self, v): self._g['obstacle_delay'] = max(1, int(v))

    @property
    def dt(self): return self._g.get('engine', {}).get('dt', 0)

    # ── Game / round info ───────────────────────────────────
    @property
    def difficulty(self): return self._g.get('difficulty', 'Mittel')

    @property
    def speed(self): return self._g.get('speed', 1.0)
    @speed.setter
    def speed(self, v): self._g['speed'] = max(0.0, float(v))

    @property
    def elapsed_time(self):
        import time as _t
        return _t.time() - self._g.get('round_start_time', _t.time())

    # ── Pickups ─────────────────────────────────────────────
    @property
    def powerups(self): return self._g.get('powerups', [])

    # ── Methods ─────────────────────────────────────────────
    def damage_player(self, amount): self._g['lives'] = max(0, self._g.get('lives', 0) - int(amount))
    def heal_player(self, amount): self._g['lives'] = self._g.get('lives', 0) + int(amount)
    def spawn_enemy(self, type='normal'): self._g.setdefault('spawn_enemy_request', []).append({'type': type})
    def spawn_powerup(self): self._g.setdefault('spawn_powerup_request', []).append({'x': self._g.get('width', 600) + 10})
    def player_hp_low(self): return self._g.get('lives', 0) <= 1
    def teleport_player(self, x, y):
        self._g['player_x'] = int(x)
        self._g['player_y'] = int(y)
    def set_velocity(self, vy): self._g['player_vy'] = float(vy)
    def add_score(self, amount): self._g['score'] = max(0, self._g.get('score', 0) + int(amount))
    def set_speed(self, value): self._g['speed'] = max(0.0, float(value))
    def flash_screen(self, duration_ms=500): self._g['flash_request'] = max(0, int(duration_ms))

class _ProAPI(_BeginnerAPI):
    @property
    def state(self): return self._g
    @property
    def enemies(self): return self._g.get('obstacles', [])
    @property
    def engine(self): return self._g.get('engine', {})
    @property
    def events(self): return self._g.get('event_bus', None)
    def get_entities(self): return self._g.get('obstacles', [])
    def clear_enemies(self): self._g.get('obstacles', []).clear()
    def remove_enemy(self, rect):
        lst = self._g.get('obstacles', [])
        if rect in lst: lst.remove(rect)
    def get_player_rect(self):
        return pygame.Rect(
            self._g.get('player_x', 0), self._g.get('player_y', 0), 50, 50)
    def emit_event(self, name, data=None):
        bus = self._g.get('event_bus')
        if bus and hasattr(bus, 'emit'): bus.emit(name, data)
    def modify_state_directly(self, key, value): self._g[key] = value

class _Mod:
    def __init__(self, meta, module, error=None):
        self.name        = meta.get('name', 'Unknown')
        self.description = meta.get('description', '')
        self.version     = meta.get('version', '1.0')
        self.author      = meta.get('author', 'Unknown')
        self.settings    = dict(meta.get('settings', {}))
        self.module      = module
        self.error       = error
        self.active      = error is None

    def _make_api(self, g):
        return _ProAPI(g)  # always full API — api_level is documentation only

    def call_hook(self, hook_name, game_ref, *extra):
        if not self.active or self.module is None: return
        fn = getattr(self.module, hook_name, None)
        if fn is None: return
        try:
            api = self._make_api(game_ref)
            game_ref['_mod_settings'] = self.settings
            fn(api, *extra)
        except Exception as exc:
            import traceback
            print(f'[MOD ERROR] {self.name}.{hook_name}: {exc}')
            traceback.print_exc()

class _ModLoader:
    def __init__(self, mods_dir):
        self.mods_dir = mods_dir
        self.mods     = []

    def load_all(self):
        if not os.path.isdir(self.mods_dir):
            print(f'[MODS] No mods/ directory at: {self.mods_dir}')
            return
        for folder in sorted(os.listdir(self.mods_dir)):
            fp = os.path.join(self.mods_dir, folder)
            if not os.path.isdir(fp): continue
            has_json = os.path.isfile(os.path.join(fp, 'mod.json'))
            has_py   = os.path.isfile(os.path.join(fp, 'mod.py'))
            if not has_json and not has_py: continue
            self._load(folder, fp)

    def _load(self, folder, fp):
        meta_path = os.path.join(fp, 'mod.json')
        py_path   = os.path.join(fp, 'mod.py')
        if not os.path.isfile(meta_path):
            err = 'ERROR: mod.json is missing'
            print(f'[MODS] {folder}: {err}')
            self.mods.append(_Mod({'name': folder}, None, error=err)); return
        if not os.path.isfile(py_path):
            err = 'ERROR: mod.py is missing'
            print(f'[MODS] {folder}: {err}')
            try:
                with open(meta_path, 'r', encoding='utf-8') as fh: meta = json.load(fh)
            except Exception: meta = {'name': folder}
            self.mods.append(_Mod(meta, None, error=err)); return
        try:
            with open(meta_path, 'r', encoding='utf-8') as fh: meta = json.load(fh)
        except Exception as exc:
            err = f'ERROR: mod.json parse failed — {exc}'
            print(f'[MODS] {folder}: {err}')
            self.mods.append(_Mod({'name': folder}, None, error=err)); return
        try:
            spec = _ilu.spec_from_file_location(f'jnd_mod_{folder}', py_path)
            mod  = _ilu.module_from_spec(spec)
            spec.loader.exec_module(mod)
        except Exception as exc:
            import traceback
            short = traceback.format_exc().strip().splitlines()[-1]
            err   = f'ERROR: mod.py import failed — {short}'
            print(f'[MODS] {folder}: {err}')
            traceback.print_exc()
            self.mods.append(_Mod(meta, None, error=err)); return
        m = _Mod(meta, mod)
        self.mods.append(m)
        print(f'[MODS] Loaded: {m.name} v{m.version} by {m.author}')

    def get_active_count(self):
        return sum(1 for m in self.mods if m.active)

    def emit(self, hook_name, game_ref, *extra):
        for m in self.mods:
            if m.active and m.error is None:
                m.call_hook(hook_name, game_ref, *extra)

# Mods-Ordner: data/mods/ bevorzugt, Fallback auf mods/ (Migration)
_MODS_DIR = os.path.join(_DATA_DIR, 'mods')
if not os.path.isdir(_MODS_DIR) and os.path.isdir(os.path.join(BASE_DIR, 'mods')):
    _MODS_DIR = os.path.join(BASE_DIR, 'mods')
mod_system = _ModLoader(_MODS_DIR)
mod_system.load_all()
_load_mod_settings()   # restore saved settings + active states
print(f'[MODS] {mod_system.get_active_count()} mod(s) active.')
_ensure_tutorial_json()  # immer beim Start data/tutorial.json anlegen/prüfen

# Monkey-patch pygame.display.flip so _draw_modded_badge() fires
# automatically on EVERY screen. The badge draws onto display_surface
# AFTER the game has blitted base_surface there, then we call the real flip.
_orig_flip = pygame.display.flip
def _patched_flip():
    # base_surface → display_surface already happened inside _jnd_flip or
    # when base_surface IS display_surface. Draw badge on top now.
    _draw_modded_badge()
    _orig_flip()
pygame.display.flip = _patched_flip

# =============================
# Game Loop
# =============================

in_menu = False
game_running = False
showing_stats = False
in_shop = False
in_loadout = False
in_profile_select = False
in_profile_name_input = False
in_mods_menu = False
in_profile_hub = False
in_online_leaderboard = False
in_player_search = False
in_public_profile = False
in_friends_hub = False
in_clan_hub = False
in_clan_create = False
in_clan_search = False
in_clan_view = False
in_chat = False
in_rename_profile = False
in_redeem_code = False

_redeem_code_text = ""
_redeem_code_msg = ""
_redeem_code_busy = False

_rename_text = ""
_rename_msg = ""
_rename_busy = False

# Online-Leaderboard UI-State (rein fürs Durchblättern - unabhängig von der
# echten Spieleinstellung difficulty_index). Nur Standard-Modus: Klassik-Läufe
# werden nicht ans Online-Leaderboard gemeldet (siehe submit_score_online()-
# Aufrufstellen in run_game()) und haben hier bewusst keine eigene Ansicht.
_lb_difficulty_idx = 0
_lb_friends_only = False
_lb_entries = None       # None = noch nie geladen, [] = geladen & leer, [...] = Einträge
_lb_loading = False
_lb_request_id = 0       # verhindert, dass eine veraltete Antwort eine neuere Auswahl überschreibt
_lb_row_rects = []       # [(rect, name), ...] - vom letzten draw_online_leaderboard()-Aufruf
_lb_my_rank = None       # (rank, score) fürs Dock unten - None solange nicht geladen,
                         # (None, None) wenn geladen aber (noch) nicht in diesem Bucket gerankt

_search_query = ""
_search_results = None
_search_loading = False
_search_request_id = 0
_search_row_rects = []

_public_profile_name = None
_public_profile_data = None
_public_profile_loading = False
_public_profile_return_screen = "in_online_leaderboard"

# Mod-menu UI state
_mods_scroll = 0
_mods_edit   = None   # (mod_index, setting_key) — active input field
_mods_edit_text = ''

# Startup: profile selection - falls ein Profil als Auto-Start markiert wurde
# (Checkbox im Profil-Screen), wird direkt dieses geladen und der Auswahl-
# Bildschirm komplett übersprungen.
_startup_profiles = get_all_profiles()
_startup_auto_name = get_auto_start_profile()
if not _startup_profiles:
    in_profile_name_input = True
elif _startup_auto_name and _startup_auto_name in _startup_profiles:
    _enter_profile(_startup_auto_name)
else:
    in_profile_select = True

session_scores        = {d: 0 for d in difficulties}
modded_session_scores = {d: 0 for d in difficulties}
session_times         = {d: 0 for d in difficulties}
modded_session_times  = {d: 0 for d in difficulties}
for diff in difficulties:
    if diff not in saved_scores:
        saved_scores[diff] = 0
    if diff not in session_scores:
        session_scores[diff] = 0
    if diff not in modded_session_scores:
        modded_session_scores[diff] = 0


# Achievements
achievement_definitions = {
    # Runden
    "first_round":        ({"Deutsch": "🏃 Erste Runde",            "English": "🏃 First Round",           "Русский": "🏃 Первый раунд",            "Türkçe": "🏃 İlk Tur",                "Schwiizerdütsch": "🏃 Erschti Rundi"},        lambda ctx: ctx["rounds"] >= 1,                            10),
    "five_rounds":        ({"Deutsch": "🔁 5 Runden gespielt",      "English": "🔁 5 Rounds Played",       "Русский": "🔁 Сыграно 5 раундов",       "Türkçe": "🔁 5 Tur Oynandı",         "Schwiizerdütsch": "🔁 5 Runde gspiilt"},      lambda ctx: ctx["rounds"] >= 5,                            25),
    "ten_rounds":         ({"Deutsch": "🎮 10 Runden gespielt",     "English": "🎮 10 Rounds Played",      "Русский": "🎮 Сыграно 10 раундов",      "Türkçe": "🎮 10 Tur Oynandı",        "Schwiizerdütsch": "🎮 10 Runde gspiilt"},     lambda ctx: ctx["rounds"] >= 10,                           50),
    "twenty_rounds":      ({"Deutsch": "🏆 20 Runden gespielt",     "English": "🏆 20 Rounds Played",      "Русский": "🏆 Сыграно 20 раундов",      "Türkçe": "🏆 20 Tur Oynandı",        "Schwiizerdütsch": "🏆 20 Runde gspiilt"},     lambda ctx: ctx["rounds"] >= 20,                           100),
    "fifty_rounds":       ({"Deutsch": "🏅 50 Runden gespielt",     "English": "🏅 50 Rounds Played",      "Русский": "🏅 Сыграно 50 раундов",      "Türkçe": "🏅 50 Tur Oynandı",        "Schwiizerdütsch": "🏅 50 Runde gspiilt"},     lambda ctx: ctx["rounds"] >= 50,                           200),
    "hundred_rounds":     ({"Deutsch": "💯 100 Runden gespielt",    "English": "💯 100 Rounds Played",     "Русский": "💯 Сыграно 100 раундов",     "Türkçe": "💯 100 Tur Oynandı",       "Schwiizerdütsch": "💯 100 Runde gspiilt"},    lambda ctx: ctx["rounds"] >= 100,                          400),
    # Score
    "score_10":           ({"Deutsch": "🎯 Aufgewärmt",             "English": "🎯 Warming Up",            "Русский": "🎯 Разминка",                "Türkçe": "🎯 Isınma",                 "Schwiizerdütsch": "🎯 Ufgwärmt"},             lambda ctx: ctx["score"] >= 10,                            15),
    "score_50":           ({"Deutsch": "🥈 Auf dem Weg",            "English": "🥈 On the Way",            "Русский": "🥈 На пути",                 "Türkçe": "🥈 Yolda",                  "Schwiizerdütsch": "🥈 Uf em Weg"},            lambda ctx: ctx["score"] >= 50,                            30),
    "score_100":          ({"Deutsch": "🥇 Zenturio",               "English": "🥇 Centurion",             "Русский": "🥇 Центурион",               "Türkçe": "🥇 Yüzlük",                 "Schwiizerdütsch": "🥇 Zenturio"},             lambda ctx: ctx["score"] >= 100,                           60),
    "score_200":          ({"Deutsch": "💎 Meisterhaft",            "English": "💎 Masterful",             "Русский": "💎 Мастерски",               "Türkçe": "💎 Ustalıkla",               "Schwiizerdütsch": "💎 Meisterhaft"},           lambda ctx: ctx["score"] >= 200,                           100),
    "score_300":          ({"Deutsch": "🔥 Unaufhaltbar",           "English": "🔥 Unstoppable",           "Русский": "🔥 Неудержимый",             "Türkçe": "🔥 Durdurulamaz",            "Schwiizerdütsch": "🔥 Unuufhaltbar"},          lambda ctx: ctx["score"] >= 300,                           150),
    "score_500":          ({"Deutsch": "👑 Legende",                "English": "👑 Legend",                "Русский": "👑 Легенда",                 "Türkçe": "👑 Efsane",                 "Schwiizerdütsch": "👑 Legendi"},               lambda ctx: ctx["score"] >= 500,                           200),
    "score_1000":         ({"Deutsch": "🌟 Göttergleich",           "English": "🌟 Godlike",               "Русский": "🌟 Богоподобный",            "Türkçe": "🌟 Tanrısal",               "Schwiizerdütsch": "🌟 Götterglych"},           lambda ctx: ctx["score"] >= 1000,                          500),
    # Schwierigkeiten
    "hardcore_start":     ({"Deutsch": "🔥 Hardcore gestartet",    "English": "🔥 Hardcore Started",      "Русский": "🔥 Хардкор начат",           "Türkçe": "🔥 Hardcore Başlatıldı",    "Schwiizerdütsch": "🔥 Hardcore gsttartet"},   lambda ctx: ctx["hardcore_started"],                       20),
    "impossible_start":   ({"Deutsch": "⚡ Unmöglich gestartet",    "English": "⚡ Impossible Started",    "Русский": "⚡ Невозможный начат",        "Türkçe": "⚡ İmkansız Başlatıldı",    "Schwiizerdütsch": "⚡ Unmöglich gsttartet"},  lambda ctx: ctx["impossible_started"],                     35),
    "impossible_survivor":({"Deutsch": "⚡ Unmöglich-Überlebender", "English": "⚡ Impossible Survivor",   "Русский": "⚡ Выживший в невозможном",   "Türkçe": "⚡ İmkansız Hayatta Kalan",  "Schwiizerdütsch": "⚡ Unmöglich-Überleber"},  lambda ctx: ctx["impossible_time"] >= 20,                  125),
    "nightmare_start":    ({"Deutsch": "💀 Albtraum gestartet",    "English": "💀 Nightmare Started",     "Русский": "💀 Кошмар начат",             "Türkçe": "💀 Kabus Başlatıldı",       "Schwiizerdütsch": "💀 Alptraum gsttartet"},   lambda ctx: ctx.get("nightmare_started", False),           50),
    "nightmare_survivor": ({"Deutsch": "💀 Albtraum-Überlebender", "English": "💀 Nightmare Survivor",    "Русский": "💀 Выживший в кошмаре",       "Türkçe": "💀 Kabus Hayatta Kalan",    "Schwiizerdütsch": "💀 Alptraum-Überleber"},   lambda ctx: ctx.get("nightmare_time", 0) >= 15,            200),
    "hell_start":         ({"Deutsch": "🔴 Hölle gestartet",       "English": "🔴 Hell Started",           "Русский": "🔴 Ад начат",                  "Türkçe": "🔴 Cehennem Başlatıldı",    "Schwiizerdütsch": "🔴 Höll gsttartet"},        lambda ctx: ctx.get("hell_started", False),                75),
    "hell_survivor":      ({"Deutsch": "🔴 Hölle-Überlebender",    "English": "🔴 Hell Survivor",          "Русский": "🔴 Выживший в аду",            "Türkçe": "🔴 Cehennem Hayatta Kalan", "Schwiizerdütsch": "🔴 Höll-Überleber"},        lambda ctx: ctx.get("hell_time", 0) >= 10,                 350),
    "dynamic_start":      ({"Deutsch": "🌀 Dynamisch gestartet",   "English": "🌀 Dynamic Started",       "Русский": "🌀 Динамика начата",          "Türkçe": "🌀 Dinamik Başlatıldı",     "Schwiizerdütsch": "🌀 Dynamisch gsttartet"},  lambda ctx: ctx.get("dynamic_started", False),             25),
    # Pickups
    "pickup_1up":         ({"Deutsch": "💚 1-UP eingesammelt",      "English": "💚 1-UP Collected",        "Русский": "💚 1-UP собрано",             "Türkçe": "💚 1-UP Toplandı",          "Schwiizerdütsch": "💚 1-UP gsammlt"},         lambda ctx: ctx["1ups"] >= 1,                              15),
    "pickup_3x_1up":      ({"Deutsch": "🍀 3x 1-UP gesamt",        "English": "🍀 3x 1-UP Total",         "Русский": "🍀 3x 1-UP всего",           "Türkçe": "🍀 Toplam 3x 1-UP",        "Schwiizerdütsch": "🍀 3x 1-UP gsamt"},        lambda ctx: ctx["1ups_total"] >= 3,                        35),
    "pickup_5_in_run":    ({"Deutsch": "💚 5x 1-UP in einer Runde","English": "💚 5x 1-UP in One Run",    "Русский": "💚 5x 1-UP за раунд",        "Türkçe": "💚 Bir Turda 5x 1-UP",     "Schwiizerdütsch": "💚 5x 1-UP id Runde"},     lambda ctx: ctx["1ups"] >= 5,                              60),
    "pickup_10x_1up":     ({"Deutsch": "🌟 10x 1-UP gesamt",       "English": "🌟 10x 1-UP Total",        "Русский": "🌟 10x 1-UP всего",          "Türkçe": "🌟 Toplam 10x 1-UP",       "Schwiizerdütsch": "🌟 10x 1-UP gsamt"},       lambda ctx: ctx["1ups_total"] >= 10,                       80),
    # Coins
    "coins_100":          ({"Deutsch": "💰 100 Coins gesammelt",    "English": "💰 100 Coins Collected",   "Русский": "💰 Собрано 100 монет",        "Türkçe": "💰 100 Para Toplandı",      "Schwiizerdütsch": "💰 100 Münze gsammlt"},    lambda ctx: ctx["total_coins"] >= 100,                     20),
    "coins_500":          ({"Deutsch": "💎 500 Coins gesammelt",    "English": "💎 500 Coins Collected",   "Русский": "💎 Собрано 500 монет",        "Türkçe": "💎 500 Para Toplandı",      "Schwiizerdütsch": "💎 500 Münze gsammlt"},    lambda ctx: ctx["total_coins"] >= 500,                     50),
    "coins_1000":         ({"Deutsch": "👑 1000 Coins gesammelt",   "English": "👑 1000 Coins Collected",  "Русский": "👑 Собрано 1000 монет",       "Türkçe": "👑 1000 Para Toplandı",     "Schwiizerdütsch": "👑 1000 Münze gsammlt"},   lambda ctx: ctx["total_coins"] >= 1000,                    100),
    "coins_2500":         ({"Deutsch": "💰 2500 Coins gesammelt",   "English": "💰 2500 Coins Collected",  "Русский": "💰 Собрано 2500 монет",       "Türkçe": "💰 2500 Para Toplandı",     "Schwiizerdütsch": "💰 2500 Münze gsammlt"},   lambda ctx: ctx["total_coins"] >= 2500,                    200),
    # Bonus
    "no_damage_run":      ({"Deutsch": "🛡 Unberührt",              "English": "🛡 Untouched",             "Русский": "🛡 Без урона",                "Türkçe": "🛡 Dokunulmamış",           "Schwiizerdütsch": "🛡 Ohni Schade"},          lambda ctx: ctx.get("no_damage") and ctx["score"] >= 20,   150),
    "speed_demon":        ({"Deutsch": "💨 Speed Demon",            "English": "💨 Speed Demon",           "Русский": "💨 Демон скорости",           "Türkçe": "💨 Hız Şeytanı",            "Schwiizerdütsch": "💨 Speed Dämon"},          lambda ctx: ctx.get("speed_bonus", False),                 75),
    "survive_60s":        ({"Deutsch": "⏱ 60 Sekunden überlebt",   "English": "⏱ Survived 60 Seconds",   "Русский": "⏱ Выжить 60 секунд",         "Türkçe": "⏱ 60 Saniye Hayatta Kal",  "Schwiizerdütsch": "⏱ 60 Sekunde überlebt"},  lambda ctx: ctx.get("survive_time", 0) >= 60,              100),
    "phoenix_revived":    ({"Deutsch": "🔮 Phönix-Wiederbelebung", "English": "🔮 Phoenix Revival",       "Русский": "🔮 Возрождение феникса",      "Türkçe": "🔮 Anka Kuşu Canlanması",   "Schwiizerdütsch": "🔮 Phönix-Wiederbeleb"},   lambda ctx: ctx.get("phoenix_revived", False),             80),
}


achievement_descriptions = {
    "first_round":       {"Deutsch": "Spiele deine erste Runde.",                          "English": "Play your first round."},
    "five_rounds":       {"Deutsch": "Spiele insgesamt 5 Runden.",                         "English": "Play 5 rounds in total."},
    "ten_rounds":        {"Deutsch": "Spiele insgesamt 10 Runden.",                        "English": "Play 10 rounds in total."},
    "twenty_rounds":     {"Deutsch": "Spiele insgesamt 20 Runden.",                        "English": "Play 20 rounds in total."},
    "fifty_rounds":      {"Deutsch": "Spiele insgesamt 50 Runden.",                        "English": "Play 50 rounds in total."},
    "hundred_rounds":    {"Deutsch": "Spiele insgesamt 100 Runden.",                       "English": "Play 100 rounds in total."},
    "score_10":          {"Deutsch": "Erreiche 10 Punkte in einer Runde.",                 "English": "Reach 10 points in a round."},
    "score_50":          {"Deutsch": "Erreiche 50 Punkte in einer Runde.",                 "English": "Reach 50 points in a round."},
    "score_100":         {"Deutsch": "Erreiche 100 Punkte in einer Runde.",                "English": "Reach 100 points in a round."},
    "score_200":         {"Deutsch": "Erreiche 200 Punkte in einer Runde.",                "English": "Reach 200 points in a round."},
    "score_300":         {"Deutsch": "Erreiche 300 Punkte in einer Runde.",                "English": "Reach 300 points in a round."},
    "score_500":         {"Deutsch": "Erreiche 500 Punkte in einer Runde.",                "English": "Reach 500 points in a round."},
    "score_1000":        {"Deutsch": "Erreiche 1000 Punkte in einer Runde.",               "English": "Reach 1000 points in a round."},
    "hardcore_start":    {"Deutsch": "Starte eine Runde im Hardcore-Modus.",               "English": "Start a round in Hardcore mode."},
    "impossible_start":  {"Deutsch": "Starte eine Runde im Unmöglich-Modus.",              "English": "Start a round in Impossible mode."},
    "impossible_survivor":{"Deutsch": "Überlebe 20 Sekunden im Unmöglich-Modus.",         "English": "Survive 20 seconds in Impossible mode."},
    "dynamic_start":     {"Deutsch": "Starte eine Runde im Dynamisch-Modus.",              "English": "Start a round in Dynamic mode."},
    "pickup_1up":        {"Deutsch": "Sammel ein 1-UP in einer Runde ein.",                "English": "Collect a 1-UP in a round."},
    "pickup_3x_1up":     {"Deutsch": "Sammel insgesamt 3 1-UPs ein.",                      "English": "Collect 3 1-UPs in total."},
    "pickup_5_in_run":   {"Deutsch": "Sammel 5 1-UPs in einer einzigen Runde.",            "English": "Collect 5 1-UPs in a single round."},
    "pickup_10x_1up":    {"Deutsch": "Sammel insgesamt 10 1-UPs ein.",                     "English": "Collect 10 1-UPs in total."},
    "coins_100":         {"Deutsch": "Verdiene insgesamt 100 Coins.",                      "English": "Earn 100 coins in total."},
    "coins_500":         {"Deutsch": "Verdiene insgesamt 500 Coins.",                      "English": "Earn 500 coins in total."},
    "coins_1000":        {"Deutsch": "Verdiene insgesamt 1000 Coins.",                     "English": "Earn 1000 coins in total."},
    "coins_2500":        {"Deutsch": "Verdiene insgesamt 2500 Coins.",                     "English": "Earn 2500 coins in total."},
    "no_damage_run":     {"Deutsch": "Beende eine Runde (min. 20 Pkt.) ohne Schaden.",     "English": "Finish a round (min. 20 pts) without taking damage."},
    "speed_demon":       {"Deutsch": "Spiele Dynamisch-Modus und erreiche den Speed-Boost.","English": "Play Dynamic mode and trigger the speed boost."},
    "survive_60s":       {"Deutsch": "Überlebe 60 Sekunden in einer Runde.",               "English": "Survive 60 seconds in a round."},
    "phoenix_revived":   {"Deutsch": "Werde durch den Phönix wiederbelebt.",               "English": "Get revived by the Phoenix power."},
}


def get_achievement_description(code):
    lang = languages[language_index]
    desc = achievement_descriptions.get(code, {})
    return desc.get(lang, desc.get("Deutsch", desc.get("English", "")))


def check_achievements(context):
    return []  # Achievements temporär deaktiviert
    newly = []
    total_coins_gained = 0
    for code, (title_dict, cond, coins) in achievement_definitions.items():
        if not achievements.get(code, False) and cond(context):
            achievements[code] = True
            title = get_achievement_name(code)
            newly.append((code, title, coins))
            total_coins_gained += coins

    if newly:
        save_section("achievements", achievements)
        # Münzen hinzufügen
        if total_coins_gained > 0:
            stats["coins"] = stats.get("coins", 0) + total_coins_gained
            save_section("stats", stats)
        for code, title, coins in newly:
            push_achievement_popup(title, coins)

    return newly


# Utility flashes (non-blocking now via overlays)
def flash_text_blocking(msg, color=(0,0,0), delay=900):
    # kept for specific blocking uses, but avoid in gameplay
    txt = menu_font.render(msg, True, color)
    base_surface.blit(txt, (WIDTH//2 - txt.get_width()//2, HEIGHT//2 - txt.get_height()//2))
    pygame.display.flip()
    pygame.time.delay(delay)


# In-game helper overlays
def show_1up_text_overlay(player_rect=None):
    pos = None
    if player_rect:
        pos = (player_rect.centerx, player_rect.top - 30)
    push_overlay("1-UP!", (0, 150, 0), duration=1.5, pos=pos)


# Compute coins on game over

def compute_coins(score, difficulty, survived_seconds):
    if klassik_mode:
        return 0
    if survived_seconds < min_survive_seconds_for_coins:
        return 0
    if difficulty not in coin_rules:
        return 0
    ratio = coin_rules[difficulty]
    coins = math.ceil(score / ratio) if isinstance(ratio, float) else score // ratio
    return coins


# Active power runtime state per run
class RuntimePowers:
    ACTIVE_NAMES = ["Slow Motion", "Time Freeze", "Ghost Mode", "Nuke"]

    def __init__(self, equipped_list):
        self.equipped = list(equipped_list)
        self.double_jump = "Double Jump" in self.equipped
        self.slow_motion = "Slow Motion" in self.equipped
        self.shield = "Shield" in self.equipped
        self.auto_dodge = "Auto Dodge" in self.equipped
        self.phoenix = "Phoenix" in self.equipped
        self.magnet = "Magnet" in self.equipped
        # cooldown tracking (ms)
        self.timers = {name: 0 for name in self.equipped}
        # effect timers (ms)
        self.slow_remaining = 0  # generic slow from Slow Motion
        self.freeze_remaining = 0  # 2s hard freeze
        self.freeze_slow_remaining = 0  # 5s slow after freeze
        self.ghost_remaining = 0  # invulnerability
        self.shield_ready = True
        self.nuke_remaining = 0  # nuke duration
        self.phoenix_cd = 0  # phoenix revival cooldown
        self.phoenix_invuln_remaining = 0  # phoenix invulnerability phase
        self.phoenix_slow_remaining = 0  # phoenix slow motion phase

    def update_cooldowns(self, dt_ms):
        for k in list(self.timers.keys()):
            if self.timers[k] > 0:
                self.timers[k] = max(0, self.timers[k] - dt_ms)
        if self.slow_remaining > 0:
            self.slow_remaining = max(0, self.slow_remaining - dt_ms)
        if self.freeze_remaining > 0:
            self.freeze_remaining = max(0, self.freeze_remaining - dt_ms)
        elif self.freeze_slow_remaining > 0:
            self.freeze_slow_remaining = max(0, self.freeze_slow_remaining - dt_ms)
        if self.ghost_remaining > 0:
            self.ghost_remaining = max(0, self.ghost_remaining - dt_ms)
        if self.nuke_remaining > 0:
            self.nuke_remaining = max(0, self.nuke_remaining - dt_ms)
        if self.phoenix_cd > 0:
            self.phoenix_cd = max(0, self.phoenix_cd - dt_ms)
        if self.phoenix_invuln_remaining > 0:
            self.phoenix_invuln_remaining = max(0, self.phoenix_invuln_remaining - dt_ms)
        if self.phoenix_slow_remaining > 0:
            self.phoenix_slow_remaining = max(0, self.phoenix_slow_remaining - dt_ms)

    def on_hit_block(self):
        # Ghost ignores hits
        if self.ghost_remaining > 0:
            return True
        # Phoenix invulnerability
        if self.phoenix_invuln_remaining > 0:
            return True
        # Shield auto block if ready
        if self.shield and self.shield_ready:
            self.shield_ready = False
            self.timers["Shield"] = POWERS_DB["Shield"]["cd"] * 1000
            return True
        return False

    def tick_shield_rearm(self):
        if self.shield and not self.shield_ready and self.timers.get("Shield",0) == 0:
            self.shield_ready = True

    def active_bindings(self):
        """Return up to two active power names in order of equipped list."""
        actives = [n for n in self.equipped if n in RuntimePowers.ACTIVE_NAMES]
        return actives[:2]
    

    def speed_multiplier(self):
        """Return a multiplier for world speed based on current effects."""
        if self.freeze_remaining > 0:
            return 0.0
        if self.freeze_slow_remaining > 0 or self.slow_remaining > 0 or self.phoenix_slow_remaining > 0:
            return 0.5
        return 1.0

    def activate(self, power_name):
        """Activate a power and return result dict for special effects like nuke."""
        if power_name not in self.equipped:
            return None

        # Check cooldown
        if power_name in self.timers and self.timers[power_name] > 0:
            return None

        result = {"nuke": False}

        if power_name == "Slow Motion":
            self.slow_remaining = POWERS_DB["Slow Motion"]["duration"] * 1000
            self.timers["Slow Motion"] = POWERS_DB["Slow Motion"]["cd"] * 1000
            push_overlay(get_text("slow_motion"), (80, 150, 255), duration=1.0)

        elif power_name == "Time Freeze":
            self.freeze_remaining = 2000  # 2 seconds
            self.freeze_slow_remaining = 5000  # 5 seconds slow after freeze
            self.timers["Time Freeze"] = POWERS_DB["Time Freeze"]["cd"] * 1000
            push_overlay(get_text("time_freeze"), (180, 80, 255), duration=1.0)

        elif power_name == "Ghost Mode":
            duration_range = POWERS_DB["Ghost Mode"]["duration"]
            duration = random.randint(duration_range[0], duration_range[1]) * 1000
            self.ghost_remaining = duration
            self.timers["Ghost Mode"] = POWERS_DB["Ghost Mode"]["cd"] * 1000
            push_overlay(get_text("ghost_mode"), (180, 180, 80), duration=1.0)

        elif power_name == "Nuke":
            self.nuke_remaining = 4000  # 4 seconds
            self.timers["Nuke"] = POWERS_DB["Nuke"]["cd"] * 1000
            push_overlay(get_text("nuke"), (200, 50, 50), duration=1.0)

        return result
        

# =============================
# Pause-Menü
# =============================

def _pause_resume_countdown(frozen, restore_vol):
    """Zeigt 1000ms eingefrorenes Spielbild mit ms-Countdown bevor resume."""
    end_time = pygame.time.get_ticks() + 1000
    clock_cd = pygame.time.Clock()
    while True:
        clock_cd.tick(60)
        remaining = max(0, end_time - pygame.time.get_ticks())
        if remaining == 0:
            break
        sync_window_size()
        base_surface.blit(frozen, (0, 0))
        _dim = pygame.Surface((WIDTH, HEIGHT), pygame.SRCALPHA)
        _dim.fill((0, 0, 0, 100))
        base_surface.blit(_dim, (0, 0))
        cd_txt = font.render(f"{remaining} ms", True, (255, 255, 255))
        base_surface.blit(cd_txt, (WIDTH // 2 - cd_txt.get_width() // 2,
                                   HEIGHT // 2 - cd_txt.get_height() // 2))
        pygame.display.flip()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
    pygame.mixer.music.set_volume(restore_vol)


def _run_pause_menu(frozen=None):
    """Zeigt das Pause-Menü als Overlay über dem eingefrorenen Spielbild.
    Gibt 'resume', 'restart' oder 'menu' zurück."""
    if frozen is None:
        frozen = base_surface.copy()

    _normal_vol = pygame.mixer.music.get_volume()
    pygame.mixer.music.set_volume(_normal_vol * 0.3)

    btn_w, btn_h, btn_gap = 280, 44, 10
    panel_w = btn_w + 60
    panel_h = 80 + 3 * (btn_h + btn_gap)

    while True:
        sync_window_size()
        base_surface.blit(frozen, (0, 0))   # eingefrorener Hintergrund

        # Dunkler Overlay
        _dim = pygame.Surface((WIDTH, HEIGHT), pygame.SRCALPHA)
        _dim.fill((0, 0, 0, 170))
        base_surface.blit(_dim, (0, 0))

        # Panel
        dark    = is_dark_mode()
        px      = WIDTH  // 2 - panel_w // 2
        py      = HEIGHT // 2 - panel_h // 2
        pan_bg  = (20, 22, 36) if dark else (238, 242, 252)
        pan_brd = (80, 110, 215) if dark else (140, 170, 225)
        pygame.draw.rect(base_surface, pan_bg,  pygame.Rect(px, py, panel_w, panel_h), border_radius=16)
        pygame.draw.rect(base_surface, pan_brd, pygame.Rect(px, py, panel_w, panel_h), width=2, border_radius=16)

        # Titel
        t = menu_font.render("Pausiert", True, get_text_color())
        base_surface.blit(t, (WIDTH // 2 - t.get_width() // 2, py + 16))

        # Buttons
        mp = get_mouse_pos_logical()
        bx = WIDTH // 2 - btn_w // 2
        by = py + 68
        _btns = [
            ("Weitermachen", "resume",  (0, 110, 50),       (255, 255, 255)),
            ("Neu starten",  "restart", get_button_color(),  get_text_color()),
            ("Hauptmenü",    "menu",    get_button_color(),  get_text_color()),
        ]
        click_rects = []
        for i, (label, action, base_col, tcol) in enumerate(_btns):
            r = pygame.Rect(bx, by + i * (btn_h + btn_gap), btn_w, btn_h)
            click_rects.append((r, action))
            hov = r.collidepoint(mp) or nav_is_focused("pause_menu", i, len(_btns))
            if action == "resume":
                bg = (0, 140, 60) if hov else base_col
            else:
                bg = get_button_hover_color() if hov else base_col
            pygame.draw.rect(base_surface, bg, r, border_radius=10)
            pygame.draw.rect(base_surface, (150, 150, 160), r, width=2, border_radius=10)
            ls = menu_font.render(label, True, tcol)
            base_surface.blit(ls, (r.centerx - ls.get_width() // 2,
                                   r.centery  - ls.get_height() // 2))

        # Hinweis
        hint = small_font.render("ESC = Weitermachen", True, (100, 100, 120))
        base_surface.blit(hint, (WIDTH // 2 - hint.get_width() // 2, py + panel_h - 22))

        pygame.display.flip()

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if action_matches_key(event.key, "pause") or is_back_key(event.key):
                    _pause_resume_countdown(frozen, _normal_vol)
                    return "resume"
                d = nav_direction(event.key)
                if d:
                    cur = nav_get_focus("pause_menu", len(click_rects))
                    nav_set_focus("pause_menu", nav_move([r for r, _a in click_rects], cur, d))
                elif is_nav_confirm(event.key):
                    _fi = nav_get_focus("pause_menu", len(click_rects))
                    if _fi is not None and _fi < len(click_rects):
                        _r, _action = click_rects[_fi]
                        play_sound(click_back_sound if _action == "menu" else menu_click_sound)
                        if _action == "resume":
                            _pause_resume_countdown(frozen, _normal_vol)
                        else:
                            pygame.mixer.music.set_volume(_normal_vol)
                        return _action
            if event.type == pygame.MOUSEBUTTONDOWN:
                mp2 = get_mouse_pos_logical()
                for r, action in click_rects:
                    if r.collidepoint(mp2):
                        play_sound(click_back_sound if action == "menu" else menu_click_sound)
                        if action == "resume":
                            _pause_resume_countdown(frozen, _normal_vol)
                        else:
                            pygame.mixer.music.set_volume(_normal_vol)
                        return action


# Main game loop per run

def draw_player_cosmetics(surface, rect, overrides=None):
    """Zeichnet die ausgerüstete Kopfbedeckung/Augen/Nase/Mund relativ zu 'rect' auf
    'surface'. Extrahiert aus run_game()'s Player-Zeichnung, damit dieselbe Optik auch
    für die Avatar-Vorschau (Profil-Hub, Game-Over-Screen) genutzt werden kann - dort
    ist 'rect' kein echter Gameplay-Spieler, sondern nur ein Vorschau-Rechteck.
    'overrides' (optional dict mit denselben equipped_*-Keys wie cosmetics_state) erlaubt
    eine HYPOTHETISCHE Vorschau (z.B. Shop-Hover: 'wie würde ich mit diesem Item aussehen',
    ohne es tatsächlich auszurüsten) - Standard ist das echte, aktuell ausgerüstete Setup."""
    c = overrides if overrides is not None else cosmetics_state
    equipped_headwear = c.get("equipped_headwear")
    equipped_eyes = c.get("equipped_eyes")
    equipped_nose = c.get("equipped_nose")
    equipped_mouth = c.get("equipped_mouth")
    if equipped_headwear == "yellow_crown":
        # Draw a simple crown above the player
        crown_x = rect.centerx - 10
        crown_y = rect.top - 5
        pygame.draw.polygon(surface, (255, 215, 0), [
            (crown_x, crown_y),
            (crown_x + 5, crown_y - 8),
            (crown_x + 10, crown_y),
            (crown_x + 15, crown_y - 8),
            (crown_x + 20, crown_y)
        ])
    elif equipped_headwear == "top_hat":
        # Zylinder: Krempe + Hutkörper + roter Streifen
        hat_x = rect.centerx
        hat_top = rect.top
        pygame.draw.rect(surface, (25, 25, 30), (hat_x - 15, hat_top - 6, 30, 6))
        pygame.draw.rect(surface, (25, 25, 30), (hat_x - 10, hat_top - 24, 20, 20))
        pygame.draw.rect(surface, (150, 20, 20), (hat_x - 10, hat_top - 9, 20, 4))
    elif equipped_headwear == "devil_horns":
        # Zwei rote Hörner über dem Spieler
        hx = rect.centerx
        hy = rect.top
        pygame.draw.polygon(surface, (210, 30, 30), [(hx - 6, hy + 4), (hx - 16, hy - 14), (hx - 4, hy - 2)])
        pygame.draw.polygon(surface, (210, 30, 30), [(hx + 6, hy + 4), (hx + 16, hy - 14), (hx + 4, hy - 2)])
    elif equipped_headwear == "halo":
        # Goldgelber, runder, leicht schwebender Ring über dem Kopf
        halo_x = rect.centerx
        halo_y = rect.top - 18
        pygame.draw.ellipse(surface, (255, 200, 20), (halo_x - 24, halo_y - 12, 48, 24), width=6)
    elif equipped_headwear == "propeller_hat":
        # Grosse Mütze mit rotierendem Propeller-Kreuz obendrauf
        ph_x = rect.centerx
        ph_top = rect.top
        pygame.draw.polygon(surface, (230, 70, 60), [
            (ph_x - 24, ph_top + 8), (ph_x + 24, ph_top + 8),
            (ph_x + 15, ph_top - 18), (ph_x - 15, ph_top - 18)
        ])
        pygame.draw.rect(surface, (230, 70, 60), (ph_x - 24, ph_top + 2, 48, 8), border_radius=3)
        _prop_angle = (pygame.time.get_ticks() / 60.0) % 360
        _prop_rad = math.radians(_prop_angle)
        _prop_dx = int(26 * math.cos(_prop_rad))
        _prop_dy = int(6 * math.sin(_prop_rad))
        pygame.draw.line(surface, (60, 60, 65),
                          (ph_x - _prop_dx, ph_top - 34 - _prop_dy), (ph_x + _prop_dx, ph_top - 34 + _prop_dy), 4)
        pygame.draw.line(surface, (60, 60, 65), (ph_x, ph_top - 44), (ph_x, ph_top - 18), 4)
        pygame.draw.circle(surface, (255, 210, 40), (ph_x, ph_top - 34), 6)
    elif equipped_headwear == "ninja_headband":
        # Breites schwarzes Stirnband mit rotem Knoten und fliegenden Enden
        nb_x = rect.centerx
        nb_y = rect.top + 6
        pygame.draw.rect(surface, (25, 25, 30), (nb_x - 26, nb_y - 8, 52, 16))
        pygame.draw.polygon(surface, (190, 20, 20), [
            (nb_x + 20, nb_y + 8), (nb_x + 42, nb_y + 26), (nb_x + 22, nb_y + 24)
        ])
        pygame.draw.polygon(surface, (190, 20, 20), [
            (nb_x + 24, nb_y + 6), (nb_x + 46, nb_y + 18), (nb_x + 26, nb_y + 20)
        ])
        pygame.draw.circle(surface, (190, 20, 20), (nb_x, nb_y), 5)

    if equipped_eyes == "pixel_eyes":
        # Blockige Pixel-Augen auf dem Spieler (bewusst nicht realistisch)
        ex = rect.centerx
        ey = rect.centery - 8
        pygame.draw.rect(surface, (255, 255, 255), (ex - 14, ey - 5, 9, 9))
        pygame.draw.rect(surface, (255, 255, 255), (ex + 5, ey - 5, 9, 9))
        pygame.draw.rect(surface, (20, 20, 25), (ex - 11, ey - 2, 4, 4))
        pygame.draw.rect(surface, (20, 20, 25), (ex + 8, ey - 2, 4, 4))
    elif equipped_eyes == "sunglasses":
        # Sonnenbrille mit dickem Rahmen, dunklen Gläsern, Glanzstreifen und Bügeln
        ex = rect.centerx
        ey = rect.centery - 8
        pygame.draw.circle(surface, (20, 20, 25), (ex - 9, ey), 10)
        pygame.draw.circle(surface, (20, 20, 25), (ex + 9, ey), 10)
        pygame.draw.circle(surface, (35, 35, 40), (ex - 9, ey), 7)
        pygame.draw.circle(surface, (35, 35, 40), (ex + 9, ey), 7)
        pygame.draw.line(surface, (150, 180, 210), (ex - 13, ey - 3), (ex - 6, ey + 3), 2)
        pygame.draw.line(surface, (150, 180, 210), (ex + 5, ey - 3), (ex + 12, ey + 3), 2)
        pygame.draw.rect(surface, (20, 20, 25), (ex - 4, ey - 2, 8, 4))
        pygame.draw.line(surface, (20, 20, 25), (ex - 19, ey - 2), (ex - 22, ey - 6), 2)
        pygame.draw.line(surface, (20, 20, 25), (ex + 19, ey - 2), (ex + 22, ey - 6), 2)
    elif equipped_eyes == "3d_glasses":
        # Klassische 3D-Brille mit rotem und blauem Glas
        ex = rect.centerx
        ey = rect.centery - 8
        pygame.draw.rect(surface, (200, 30, 30), (ex - 16, ey - 7, 13, 11))
        pygame.draw.rect(surface, (40, 120, 220), (ex + 3, ey - 7, 13, 11))
        pygame.draw.rect(surface, (20, 20, 25), (ex - 4, ey - 2, 8, 3))
    elif equipped_eyes == "robot_visor":
        # Durchgehendes, leuchtend blaues Visier über beiden Augen
        ex = rect.centerx
        ey = rect.centery - 8
        pygame.draw.rect(surface, (30, 30, 35), (ex - 17, ey - 7, 34, 15), border_radius=6)
        pygame.draw.rect(surface, (80, 220, 255), (ex - 13, ey - 3, 26, 7), border_radius=3)

    if equipped_nose == "clown_nose":
        # Runde rote Clown-Nase mittig im Gesicht
        nx = rect.centerx
        ny = rect.centery
        pygame.draw.circle(surface, (230, 30, 30), (nx, ny), 6)
        pygame.draw.circle(surface, (150, 10, 10), (nx, ny), 6, width=1)
    elif equipped_nose == "pig_nose":
        # Rosa Rüsselscheibe mit zwei ovalen Nüstern
        nx = rect.centerx
        ny = rect.centery
        pygame.draw.ellipse(surface, (240, 150, 170), (nx - 9, ny - 6, 18, 13))
        pygame.draw.ellipse(surface, (190, 100, 120), (nx - 9, ny - 6, 18, 13), width=1)
        pygame.draw.ellipse(surface, (190, 100, 120), (nx - 5, ny - 2, 4, 5))
        pygame.draw.ellipse(surface, (190, 100, 120), (nx + 1, ny - 2, 4, 5))

    if equipped_mouth == "big_grin":
        # Wemmbu Smile: pixelig/blockig im Minecraft-Stil (eckige U-Klammer, dicke Wände)
        mx = rect.centerx
        my = rect.centery + 10
        _wpx = 2
        _wpat = [
            "1..........1",
            "1..........1",
            "1..........1",
            "1..........1",
            "1..........1",
            "111111111111",
        ]
        for _wr, _wrow in enumerate(_wpat):
            for _wc, _wch in enumerate(_wrow):
                if _wch == "1":
                    pygame.draw.rect(surface, (10, 10, 15),
                                     (mx - 13 * _wpx // 2 + _wc * _wpx, my - 3 * _wpx + _wr * _wpx, _wpx, _wpx))
    elif equipped_mouth == "vampire_fangs":
        # Zwei spitze weisse Reisszähne unter einem dunklen Lächeln-Strich
        mx = rect.centerx
        my = rect.centery + 10
        pygame.draw.line(surface, (10, 10, 15), (mx - 8, my), (mx + 8, my), 2)
        pygame.draw.polygon(surface, (255, 255, 250), [(mx - 6, my), (mx - 2, my), (mx - 4, my + 7)])
        pygame.draw.polygon(surface, (255, 255, 250), [(mx + 2, my), (mx + 6, my), (mx + 4, my + 7)])
    elif equipped_mouth == "nerd_teeth":
        # Zwei breite, quadratische Hasenzähne mit Mittelspalt
        mx = rect.centerx
        my = rect.centery + 10
        pygame.draw.line(surface, (10, 10, 15), (mx - 10, my), (mx + 10, my), 2)
        pygame.draw.rect(surface, (255, 255, 250), (mx - 9, my, 8, 11), border_radius=2)
        pygame.draw.rect(surface, (255, 255, 250), (mx + 1, my, 8, 11), border_radius=2)
        pygame.draw.rect(surface, (200, 200, 195), (mx - 9, my, 8, 11), width=1, border_radius=2)
        pygame.draw.rect(surface, (200, 200, 195), (mx + 1, my, 8, 11), width=1, border_radius=2)
    elif equipped_mouth == "bird_beak":
        # Spitzer, gelber Vogelschnabel über dem Mund (Ober- und Unterschnabel)
        mx = rect.centerx
        my = rect.centery + 10
        pygame.draw.polygon(surface, (240, 170, 30), [(mx - 12, my - 3), (mx + 13, my), (mx - 10, my + 4)])
        pygame.draw.polygon(surface, (200, 140, 15), [(mx - 12, my + 2), (mx + 9, my + 4), (mx - 10, my + 9)])
        pygame.draw.line(surface, (150, 100, 5), (mx - 11, my + 1), (mx + 8, my + 3), 1)


PLAYER_COLORS = {
    # Bewusst KEINE Farben, die schon für Gameplay-Elemente stehen (RED = normale
    # Hindernisse, GREEN = 1-UP-Powerups, PURPLE-artig = Top-Hindernisse) - sonst
    # verschmilzt der eigene Avatar optisch mit Hindernissen/Pickups. Ausnahme: das alte
    # Standard-Dunkelblau ist weiterhin dabei (und Default), das kollidiert mit nichts.
    "blue":       BLUE,            # Standard
    "darkgreen":  (20, 100, 40),
    "lightblue":  (80, 180, 255),
    "pink":       (255, 105, 180),
    "orange":     (255, 120, 0),   # gleiche Farbe wie Spieler 2 im 2-Player-Modus
    "yellow":     GOLD,
}

def get_player_color():
    """Gibt die vom Nutzer gewählte Avatar-/Spielerfarbe zurück (Settings, pro Profil). Dunkelblau ist der Standard."""
    return PLAYER_COLORS.get(settings.get("player_color", "blue"), BLUE)

def draw_avatar(surface, rect, color=None, cosmetics=True, cosmetics_overrides=None):
    """Zeichnet den Avatar: farbiger Block (wie der echte Spieler) + alle ausgerüsteten
    Nicht-Trail-Kosmetik-Items (Kopfbedeckung/Augen/Nase/Mund) - Trails ergeben auf einem
    statischen Avatar keinen Sinn und werden bewusst nicht gezeichnet. Wiederverwendet für
    Profil-Hub-Vorschau und Game-Over-Screen (dort in echter Spielergröße 50x50, damit
    Hüte/Augen/etc. exakt wie im Gameplay aussehen - draw_player_cosmetics() positioniert
    sie mit dafür fest abgestimmten Pixel-Offsets). cosmetics=False für sehr kleine
    Vorschauen (z.B. das Mini-Icon im Hauptmenü), wo Hüte etc. sonst über den verfügbaren
    Platz hinaus in benachbarte UI-Elemente ragen würden. cosmetics_overrides siehe
    draw_player_cosmetics() - für hypothetische "so würde es aussehen"-Vorschauen
    (Cosmetic-Shop-Hover), ohne tatsächlich etwas auszurüsten."""
    pygame.draw.rect(surface, color if color is not None else get_player_color(), rect, border_radius=8)
    if cosmetics:
        draw_player_cosmetics(surface, rect, overrides=cosmetics_overrides)


def run_game():
    global stats, flash_timer, modded_scores, modded_stats, game_running, in_menu
    clock = pygame.time.Clock()
    if current_game_mode == "custom":
        difficulty = "Custom"
        speed = settings.get("custom_speed", 10)
        min_delay = settings.get("custom_min_delay", 900)
        max_delay = settings.get("custom_max_delay", 1600)
    else:
        difficulty = difficulties[difficulty_index]
        speed = difficulty_speeds[difficulty]
        min_delay, max_delay = difficulty_obstacle_intervals[difficulty]

    # Play ingame music
    play_ingame_music()

    dynamic_mode = (difficulty == "Dynamisch")
    dynamic_timer = 0
    dynamic_increase_interval = 5000
    dynamic_speed_increment = 0.5

    player = pygame.Rect(50, HEIGHT - 50, 50, 50)
    player_velocity_y = 0

    # Trail particles for all cosmetic trails (ein Partikel-Puffer je Trail-Item,
    # ALL_TRAIL_KEYS wächst automatisch mit COSMETICS_DB mit)
    trail_particle_lists = {key: [] for key in ALL_TRAIL_KEYS}
    fire_particles = trail_particle_lists["fire_trail"]
    ice_particles = trail_particle_lists["ice_trail"]
    shadow_particles = trail_particle_lists["shadow_trail"]
    sparkle_particles = trail_particle_lists["sparkle_trail"]
    toxic_particles = trail_particle_lists["toxic_trail"]
    electric_particles = trail_particle_lists["electric_trail"]
    PARTICLE_SPEED = 2

    # Fire trail effect
    fire_trail = []  # List of past positions (x, y, age)
    TRAIL_LENGTH = 10
    TRAIL_FADE = 255 / TRAIL_LENGTH
    gravity = 1
    is_jumping = False
    used_double_jump = False
    move_speed = 8

    # Almost Dead effect state
    heartbeat_timer = 0
    heartbeat_interval = 850  # ms between beats
    almost_dead_pulse = 0.0   # accumulated ms for vignette sine pulse

    obstacles = []
    top_obstacles = []
    powerups = []  # 1-Ups
    obstacle_timer = 0
    obstacle_delay_bag = ObstacleDelayBag()
    obstacle_delay = obstacle_delay_bag.next(min_delay, max_delay)
    top_obstacle_timer = 0
    top_obstacle_delay = int(random.randint(900, 1600) * (800 / max(400, WIDTH)))
    score = 0
    lives = 3
    oneups_collected = 0
    hit_invuln = 0   # ms Unverwundbarkeit nach einem Treffer

    # Wind-Modifier (nur Custom-Modus): zwei Varianten -
    # "Wind (Seiten)" = nur horizontal (der alte Wind), "Wind (Allseitig)" = horizontal + vertikal
    _custom_mode_active = current_game_mode == "custom"
    wind_h_enabled = _custom_mode_active and (settings.get("custom_wind_sides", False) or settings.get("custom_wind_all", False))
    wind_v_enabled = _custom_mode_active and settings.get("custom_wind_all", False)

    # Free-Roam-Modifier (nur Custom-Modus): freie WASD-Bewegung statt Sprung/Schwerkraft,
    # Hindernisse kommen aus allen Richtungen statt nur von rechts/oben
    free_roam_enabled = _custom_mode_active and settings.get("custom_free_roam", False)
    FREE_ROAM_SPEED = 10
    wind_h_strength = 0.0    # -1.0 (links) .. 1.0 (rechts), geglättet
    wind_h_target = 0.0
    wind_h_phase_timer = 0
    wind_v_strength = 0.0    # -1.0 (oben) .. 1.0 (unten), geglättet
    wind_v_target = 0.0
    wind_v_phase_timer = 0
    # Fester Partikel-Pool, von Anfang an über den GANZEN Bildschirm verteilt (nicht nur an
    # einer Kante), damit der Effekt sofort überall sichtbar ist statt erst reinzufliegen.
    wind_particles = [[random.randint(0, WIDTH), random.randint(0, HEIGHT), random.randint(30, 70)]
                       for _ in range(26)] if (wind_h_enabled or wind_v_enabled) else []

    def _wind_roll_phase():
        """Würfelt Ziel-Stärke/-Richtung + Dauer für eine Wind-Achse (Ruhe/leichte/starke Böe)."""
        roll = random.random()
        if roll < 0.3:
            return 0.0, random.randint(1200, 3000)
        elif roll < 0.7:
            return random.uniform(0.25, 0.55) * random.choice([-1, 1]), random.randint(1800, 4000)
        else:
            return random.uniform(0.6, 1.0) * random.choice([-1, 1]), random.randint(1200, 2800)

    round_start_time = time.time()

    # ------------------------------------------------------------------
    # Mod system: shared game-state dict updated each frame
    # ------------------------------------------------------------------
    _mod_game_ref = {
        'lives':               lives,
        'score':               score,
        'inventory':           powers_state.get('equipped', []),
        'obstacles':           obstacles,
        'powerups':            powerups,
        'spawn_enemy_request': [],
        'engine': {
            'speed':      speed,
            'difficulty': difficulty,
            'dt':         0,
        },
        'event_bus': None,
        # Player transform
        'player_x':         player.x,
        'player_y':         player.y,
        'player_vy':        0,
        'is_jumping':       is_jumping,
        'used_double_jump': used_double_jump,
        # Physics
        'move_speed':       move_speed,
        'gravity':          gravity,
        'speed':            speed,
        # Obstacle spawning
        'obstacle_timer':   obstacle_timer,
        'obstacle_delay':   obstacle_delay,
        # Spawn requests
        'spawn_powerup_request': [],
        # Screen (read-only)
        'width':            WIDTH,
        'height':           HEIGHT,
        # Round info (read-only)
        'round_start_time': round_start_time,
        'difficulty':       difficulty,
        # Effects
        'flash_request':    0,
    }

    def _mod_sync_in():
        """Copy local game vars into the shared mod dict (before hooks)."""
        _mod_game_ref['lives']            = lives
        _mod_game_ref['score']            = score
        _mod_game_ref['engine']['speed']  = speed
        _mod_game_ref['engine']['dt']     = 0
        _mod_game_ref['spawn_enemy_request'] = []
        _mod_game_ref['player_x']         = player.x
        _mod_game_ref['player_y']         = player.y
        _mod_game_ref['player_vy']        = player_velocity_y
        _mod_game_ref['is_jumping']       = is_jumping
        _mod_game_ref['used_double_jump'] = used_double_jump
        _mod_game_ref['move_speed']       = move_speed
        _mod_game_ref['gravity']          = gravity
        _mod_game_ref['obstacle_timer']   = obstacle_timer
        _mod_game_ref['obstacle_delay']   = obstacle_delay
        _mod_game_ref['speed']            = speed
        _mod_game_ref['width']            = WIDTH
        _mod_game_ref['height']           = HEIGHT
        _mod_game_ref['round_start_time'] = round_start_time
        _mod_game_ref['difficulty']       = difficulty
        _mod_game_ref['spawn_powerup_request'] = []
        _mod_game_ref['flash_request']    = 0

    def _mod_sync_out():
        """Apply mod-written values back to local game vars + handle spawn requests."""
        nonlocal lives, score, player_velocity_y, is_jumping, used_double_jump
        nonlocal move_speed, gravity, obstacle_timer, obstacle_delay, speed
        global flash_timer
        lives             = _mod_game_ref['lives']
        score             = _mod_game_ref['score']
        player.x          = int(_mod_game_ref['player_x'])
        player.y          = int(_mod_game_ref['player_y'])
        player_velocity_y = _mod_game_ref['player_vy']
        is_jumping        = _mod_game_ref['is_jumping']
        used_double_jump  = _mod_game_ref['used_double_jump']
        move_speed        = _mod_game_ref['move_speed']
        gravity           = _mod_game_ref['gravity']
        speed             = max(0.0, float(_mod_game_ref['speed']))
        _mod_game_ref['engine']['speed'] = speed
        obstacle_timer    = _mod_game_ref['obstacle_timer']
        obstacle_delay    = _mod_game_ref['obstacle_delay']
        # Flash request
        if _mod_game_ref.get('flash_request', 0) > 0:
            flash_timer = int(_mod_game_ref['flash_request'])
            _mod_game_ref['flash_request'] = 0
        # Spawn enemy requests
        for req in _mod_game_ref.get('spawn_enemy_request', []):
            etype = req.get('type', 'normal') if isinstance(req, dict) else 'normal'
            sizes = {'large': (75, 75), 'small': (30, 30)}
            w, h  = sizes.get(etype, (50, 50))
            obstacles.append(pygame.Rect(WIDTH + 10, HEIGHT - h, w, h))
        _mod_game_ref['spawn_enemy_request'] = []
        # Spawn powerup requests
        for req in _mod_game_ref.get('spawn_powerup_request', []):
            px = req.get('x', WIDTH + 10) if isinstance(req, dict) else WIDTH + 10
            powerups.append(pygame.Rect(px, HEIGHT - 100, 50, 50))
        _mod_game_ref['spawn_powerup_request'] = []

    # Snapshot BEFORE any mod hooks — used to route stat saves, cannot be bypassed by mods
    _mods_were_active = mod_system.get_active_count() > 0

    # Fire on_game_start once
    _mod_sync_in()
    mod_system.emit('on_game_start', _mod_game_ref)
    _mod_sync_out()

    # Active powers this run (apply booster early)
    equipped = powers_state.get("equipped", [])[:get_max_slots()]
    rp = RuntimePowers(equipped)

    # Achievement context
    ctx = {
        "rounds": stats["total_rounds"] + 1,
        "score": 0,
        "1ups": 0,
        "1ups_total": stats["total_1ups"],
        "hardcore_started": difficulty == "Hardcore",
        "hardcore_time": 0,
        "impossible_started": difficulty == "Unmöglich",
        "impossible_time": 0,
        "nightmare_started": difficulty == "Albtraum",
        "nightmare_time": 0,
        "hell_started": difficulty == "Hölle",
        "hell_time": 0,
        "total_coins": stats.get("coins", 0),
        "no_damage": True,
        "speed_bonus": False,
    }
    if ctx["hardcore_started"] or ctx.get("nightmare_started") or ctx.get("hell_started"):
        check_achievements(ctx)

    # Key bindings for actives
    def handle_activation(key):
        binds = rp.active_bindings()
        if not binds:
            return None
        if action_matches_key(key, "power1") and len(binds) >= 1:
            return rp.activate(binds[0])
        if action_matches_key(key, "power2") and len(binds) >= 2:
            return rp.activate(binds[1])
        return None

    # To avoid repeated jumping when holding space, detect edge
    space_was_down = False

    # Ensure used_double_jump resets at start
    used_double_jump = False

    _last_game_frame = base_surface.copy()

    while True:
        sync_window_size()

        # Normalerweise setzt kein Code-Pfad game_running=False, ohne direkt danach
        # aus run_game() zurückzukehren (siehe Pause-Menü "Zurück zum Menü"). Wird es
        # trotzdem False vorgefunden, kam das von aussen (Dev-Konsole, z.B. "run menu"
        # während einer laufenden Runde) - dann sauber verlassen statt weiterzuspielen.
        if not game_running:
            stop_music()
            return

        dt = clock.tick(60)

        # --- Dev-Konsole: In-Runde-Aktionen anwenden (score/lives/etc. sind lokale
        # Variablen dieser Funktion, deshalb muss run_game() sie selbst anwenden -
        # siehe _dev_cmd_round_action, das sie nur in _dev_round_actions einreiht) ---
        for _dev_action, _dev_arg in _drain_dev_round_actions():
            if _dev_action == "collect_1up":
                _n = _dev_arg or 1
                lives += _n
                oneups_collected += _n
                stats["total_1ups"] = stats.get("total_1ups", 0) + _n
                print(f"[DEV] collect_1up({_n}) angewendet: lives={lives}, oneups_collected={oneups_collected}")
            elif _dev_action == "add_score":
                score += _dev_arg or 0
                print(f"[DEV] add_score({_dev_arg}) angewendet: score={score}")
            elif _dev_action == "add_lives":
                lives += _dev_arg or 0
                print(f"[DEV] add_lives({_dev_arg}) angewendet: lives={lives}")
            elif _dev_action == "set_lives":
                if _dev_arg is not None:
                    lives = _dev_arg
                print(f"[DEV] set_lives({_dev_arg}) angewendet: lives={lives}")
            elif _dev_action == "god_mode":
                hit_invuln = 999999999 if _dev_arg else 0
                print(f"[DEV] god_mode({'on' if _dev_arg else 'off'}) angewendet: hit_invuln={hit_invuln}")

        # --- Mod on_update hook ---
        _mod_sync_in()
        _mod_game_ref['engine']['dt'] = dt
        mod_system.emit('on_update', _mod_game_ref)
        _mod_sync_out()
        # --------------------------

        if flash_timer > 0:
            flash_timer -= dt
        fill_bg(rings=True)

        # Violette Boden-Linie auf Höhe der tatsächlichen Boden-Physik (HEIGHT - 50, siehe
        # Landing-Klemme weiter unten) - abschaltbar in den Einstellungen, im Free-Roam-
        # Modifier ohnehin ausgeblendet (da gibt's keinen festen Boden).
        if settings.get("show_floor_line", True) and not free_roam_enabled:
            floor_line_y = HEIGHT - 48
            pygame.draw.line(base_surface, (60, 30, 110), (0, floor_line_y - 1), (WIDTH, floor_line_y - 1), 1)
            pygame.draw.line(base_surface, (140, 80, 220), (0, floor_line_y), (WIDTH, floor_line_y), 2)

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if action_matches_key(event.key, "pause"):
                    _pause_t = time.time()
                    _pause_result = _run_pause_menu(frozen=_last_game_frame)
                    round_start_time += time.time() - _pause_t
                    clock.tick()  # accumulated dt verwerfen
                    if _pause_result == "menu":
                        stop_music()
                        game_running = False
                        in_menu = True
                        return
                    elif _pause_result == "restart":
                        stop_music()
                        return   # main loop ruft run_game() erneut auf
                    # "resume": einfach weitermachen
                else:
                    res = handle_activation(event.key)
        
        keys = pygame.key.get_pressed()
        # Jump handling with edge detection and reliable double jump (nicht im Free-Roam-Modifier)
        if not free_roam_enabled:
            space_down = any_action_pressed(keys, "jump")
            if space_down and not space_was_down:
                # space key pressed (edge)
                if not is_jumping:
                    player_velocity_y = -20
                    is_jumping = True
                    used_double_jump = False
                    stats["total_jumps"] = stats.get("total_jumps", 0) + 1
                elif rp.double_jump and not used_double_jump:
                    player_velocity_y = -18
                    used_double_jump = True
                    stats["total_jumps"] = stats.get("total_jumps", 0) + 1
                    push_overlay(f"{get_power_name('Double Jump')}!", (120,200,255), duration=1.0, pos=(player.centerx, player.top-20))
            space_was_down = space_down

        # ---- Wind-Modifier: horizontale + vertikale Achse unabhängig in Phasen ----
        # Pausiert komplett während Time Freeze, genau wie Hindernisse/Spawns (rp.freeze_remaining).
        if rp.freeze_remaining == 0:
            if wind_h_enabled:
                wind_h_phase_timer -= dt
                if wind_h_phase_timer <= 0:
                    wind_h_target, wind_h_phase_timer = _wind_roll_phase()
                wind_h_strength += (wind_h_target - wind_h_strength) * 0.02
            if wind_v_enabled:
                wind_v_phase_timer -= dt
                if wind_v_phase_timer <= 0:
                    wind_v_target, wind_v_phase_timer = _wind_roll_phase()
                wind_v_strength += (wind_v_target - wind_v_strength) * 0.02

        if free_roam_enabled:
            # Freie Bewegung in alle Richtungen mit WASD, keine Schwerkraft
            if keys[pygame.K_w]:
                player.y -= FREE_ROAM_SPEED
            if keys[pygame.K_s]:
                player.y += FREE_ROAM_SPEED
            if keys[pygame.K_a]:
                player.x -= FREE_ROAM_SPEED
            if keys[pygame.K_d]:
                player.x += FREE_ROAM_SPEED
            if wind_h_enabled and rp.freeze_remaining == 0:
                player.x += wind_h_strength * 4.5
            if wind_v_enabled and rp.freeze_remaining == 0:
                player.y += wind_v_strength * 3.5
            if player.top < 0:
                player.top = 0
            if player.bottom > HEIGHT:
                player.bottom = HEIGHT
        else:
            if any_action_pressed(keys, "left"):
                player.x -= move_speed
            if any_action_pressed(keys, "right"):
                player.x += move_speed
            if wind_h_enabled and rp.freeze_remaining == 0:
                player.x += wind_h_strength * 4.5

        if player.left < 0:
            player.left = 0
        if player.right > WIDTH:
            player.right = WIDTH

        # Update trail particles for all equipped trails
        equipped_trails = [t for t in ALL_TRAIL_KEYS
                          if t in cosmetics_state.get("owned", []) and t in cosmetics_state.get("equipped", [])]

        # Add new particles only when moving and trails are equipped
        if equipped_trails and (any_action_pressed(keys, "left") or any_action_pressed(keys, "right") or player_velocity_y < 0):
            for trail in equipped_trails:
                particles = trail_particle_lists[trail]

                # Different particle counts for different trails
                particle_count = {
                    "fire_trail": 5,      # Dense fire particles
                    "ice_trail": 4,       # Medium ice particles
                    "shadow_trail": 3,    # Sparse shadow particles
                    "sparkle_trail": 6,   # Lots of sparkles
                    "toxic_trail": 4,     # Medium toxic particles
                    "electric_trail": 5,  # Dense, snappy sparks
                    "rainbow_trail": 6,   # Legendary: viele bunte Partikel
                    "star_trail": 4,      # Ruhig treibende Sterne
                    "smoke_trail": 3,     # Sparse, langsam wachsender Rauch
                    "blood_trail": 4,     # Tropfen, die nach unten fallen
                    "heart_trail": 4      # Herzen, die sanft nach oben schweben
                }.get(trail, 3)

                for _ in range(particle_count):
                    # Determine direction for spawn
                    if any_action_pressed(keys, "left"):
                        spawn_x = player.right + random.uniform(-5, 5)
                        vx = random.uniform(-3, -0.5)  # Leftward trail
                    elif any_action_pressed(keys, "right"):
                        spawn_x = player.left + random.uniform(-5, 5)
                        vx = random.uniform(0.5, 3)   # Rightward trail
                    else:
                        spawn_x = player.centerx + random.uniform(-8, 8)
                        vx = random.uniform(-2, 2)

                    # Different spawn patterns for different trails
                    if trail == "sparkle_trail":
                        vy = random.uniform(-2, 1)  # Sparkles float upward
                        life = random.uniform(30, 60)  # Longer life for sparkles
                    elif trail == "shadow_trail":
                        vy = random.uniform(-0.5, 0.5)  # Shadow particles stay level
                        life = random.uniform(25, 45)
                    elif trail == "ice_trail":
                        vy = random.uniform(-1.5, 0.5)  # Ice particles float gently
                        life = random.uniform(35, 55)
                    elif trail == "toxic_trail":
                        vy = random.uniform(-1, 0)     # Toxic particles drift slightly up
                        life = random.uniform(40, 65)
                    elif trail == "electric_trail":
                        vy = random.uniform(-1.5, 1.5)  # Sparks jitter erratically
                        life = random.uniform(18, 32)   # Short, snappy zaps
                    elif trail == "rainbow_trail":
                        vy = random.uniform(-1.5, 0.5)  # Legendary Partikel schweben leicht nach oben
                        life = random.uniform(35, 60)
                    elif trail == "star_trail":
                        vy = random.uniform(-1.5, 0.5)  # Sterne treiben ruhig nach oben
                        life = random.uniform(35, 55)
                    elif trail == "smoke_trail":
                        vy = random.uniform(-1.2, -0.2)  # Rauch steigt gleichmäßig auf
                        life = random.uniform(40, 70)
                    elif trail == "blood_trail":
                        vy = random.uniform(0, 1.5)     # Blutstropfen fallen nach unten
                        life = random.uniform(18, 32)
                    elif trail == "heart_trail":
                        vy = random.uniform(-1.8, -0.3)  # Herzen schweben nach oben
                        life = random.uniform(30, 55)
                    else:  # fire_trail
                        vy = random.uniform(-1, 0)      # Fire particles with upward bias
                        life = random.uniform(20, 40)

                    particle_data = {
                        'x': spawn_x,
                        'y': player.centery + random.uniform(-8, 8),  # Spread vertically
                        'vx': vx,
                        'vy': vy,
                        'life': life,
                        'max_life': life,
                        'trail_type': trail,
                        'size': random.uniform(1.5, 3.5),  # Variable sizes
                        'angle': random.uniform(0, 360)    # For rotation effects
                    }
                    particles.append(particle_data)

        # Update all particle types with enhanced physics
        for particles in trail_particle_lists.values():
            particles[:] = [p for p in particles if p['life'] > 0]
            for p in particles:
                p['x'] += p['vx']
                p['y'] += p['vy']

                # Different physics for different trails
                if p['trail_type'] == "sparkle_trail":
                    p['vy'] += 0.05  # Sparkles float up slowly
                    p['angle'] = (p.get('angle', 0) + 5) % 360  # Rotate sparkles
                elif p['trail_type'] == "shadow_trail":
                    p['vy'] += 0.02  # Shadow particles fall slowly
                elif p['trail_type'] == "ice_trail":
                    p['vy'] += 0.08   # Ice particles have light gravity
                    p['vx'] *= 0.98   # Ice particles slow down horizontally
                elif p['trail_type'] == "toxic_trail":
                    p['vy'] += 0.06   # Toxic particles drift
                    p['vx'] *= 0.99   # Slight air resistance
                elif p['trail_type'] == "electric_trail":
                    p['vx'] += random.uniform(-0.15, 0.15)  # Erratic crackling jitter
                    p['vy'] += random.uniform(-0.15, 0.15)
                elif p['trail_type'] == "rainbow_trail":
                    p['vy'] += 0.03  # Legendary Partikel schweben sanft
                    p['angle'] = (p.get('angle', 0) + 4) % 360  # Farbrotation für Regenbogen-Effekt
                elif p['trail_type'] == "star_trail":
                    p['vy'] += 0.04  # Sterne treiben ruhig nach oben
                    p['angle'] = (p.get('angle', 0) + 3) % 360  # Funkeln
                elif p['trail_type'] == "smoke_trail":
                    p['vy'] += 0.015  # Rauch steigt langsam und gleichmäßig
                    p['vx'] *= 0.985  # Rauch verliert horizontalen Schwung
                    p['size'] = p.get('size', 2.5) + 0.03  # Rauch wird beim Aufsteigen größer
                elif p['trail_type'] == "blood_trail":
                    p['vy'] += 0.18   # Blutstropfen fallen deutlich schneller nach unten
                elif p['trail_type'] == "heart_trail":
                    p['vy'] += 0.02   # Herzen schweben leicht nach oben
                    p['vx'] += math.sin(p.get('angle', 0) * 0.05) * 0.05  # sanftes Schlängeln
                else:  # fire_trail
                    p['vy'] += 0.1    # Fire particles have normal gravity

                p['life'] -= 1

                # Gravity (nicht im Free-Roam-Modifier - da gibt's keinen Boden/keine Schwerkraft)
        if not free_roam_enabled:
            player_velocity_y += gravity
            if wind_v_enabled and rp.freeze_remaining == 0:
                # Wind von oben (negativ) bremst den Fall spürbar ab (man wird "getragen"),
                # Wind von unten (positiv) verstärkt den Fall - wirkt auf die Geschwindigkeit,
                # nicht nur auf die Position, damit es sich in den Sprung/Fall-Bogen einfügt.
                player_velocity_y += wind_v_strength * 0.6
            player.y += player_velocity_y
            if player.top < 0:
                player.top = 0
                player_velocity_y = 0

            if player.bottom >= HEIGHT - 50:
                player.bottom = HEIGHT - 50
                player_velocity_y = 0

                # Beim Landen Sprung-Status komplett zurücksetzen
                if is_jumping or used_double_jump:
                    is_jumping = False
                    used_double_jump = False

                # Edge-Detection zurücksetzen, damit sofort wieder gesprungen werden kann
                space_was_down = False


        # Spawn (paused while hard freeze)
        if rp.freeze_remaining == 0:
            obstacle_timer += dt
        if obstacle_timer >= obstacle_delay:
            obstacle_timer = 0
            mult = rp.speed_multiplier()
            obstacle_delay = int(obstacle_delay_bag.next(min_delay, max_delay) / mult) if mult > 0 else 999999
            oneup_chance = 0.015 * (1.15 if "Medic" in rp.equipped else 1.0)
            if difficulty == "Custom" and not settings.get("custom_oneups", True):
                oneup_chance = 0.0
            spawn_x = WIDTH
            if random.random() < oneup_chance:
                powerups.append(pygame.Rect(spawn_x, HEIGHT - 100, 50, 50))
            elif free_roam_enabled:
                obstacles.append(make_free_roam_obstacle())
            else:
                obstacles.extend(make_obstacle(spawn_x, pick_obstacle_type(difficulty, speed if dynamic_mode else None)))

        # Top obstacles (harder difficulties, not Klassik; im Free-Roam-Modifier kommen
        # Hindernisse ohnehin schon von oben/unten/links/rechts, daher hier deaktiviert)
        _top_active = not klassik_mode and not free_roam_enabled and (
            difficulty in {"Hardcore", "Unmöglich", "Albtraum", "Hölle"}
            or (dynamic_mode and speed >= 14)
            or (difficulty == "Custom" and settings.get("custom_vertical_obstacles", False))
        )
        if _top_active:
            if rp.freeze_remaining == 0:
                top_obstacle_timer += dt
            if top_obstacle_timer >= top_obstacle_delay:
                top_obstacle_timer = 0
                top_obstacles.append(make_top_obstacle())
                top_obstacle_delay = int(random.randint(900, 1600) * (800 / max(400, WIDTH)))

        # Speed adjustment by effects
        current_speed = speed * rp.speed_multiplier()
        fall_speed = max(2.0, current_speed * 0.85)

        # Move & score
        for obs in obstacles[:]:
            is_free_roam_obs = obs.dir_x is not None or obs.dir_y is not None
            if is_free_roam_obs:
                # Free-Roam-Hindernis: feste Richtung von einer zufälligen Kante,
                # current_speed ist während Time Freeze bereits 0 (rp.speed_multiplier())
                obs.x += obs.dir_x * current_speed
                obs.y += obs.dir_y * current_speed
                if rp.freeze_remaining == 0:
                    if wind_h_enabled:
                        obs.x += wind_h_strength * 5.0
                    if wind_v_enabled:
                        obs.y += wind_v_strength * 4.5
            else:
                obs.x -= current_speed
                if wind_h_enabled and rp.freeze_remaining == 0:
                    obs.x += wind_h_strength * 5.0  # horizontaler Wind wirkt auf Boden-Hindernisse (horizontale Bewegung)
            # Normale Boden-Hindernisse bewegen sich nur nach links, daher reicht "links raus".
            # "obs.left > WIDTH" NICHT für sie prüfen: eine Power, die sie weiter rechts als
            # WIDTH spawnen lässt (z.B. für mehr Vorwarnzeit), würde sie sonst im allerersten
            # Frame nach dem Spawnen sofort wieder löschen, bevor sie je sichtbar werden.
            # Free-Roam-Hindernisse können sich in jede Richtung bewegen, brauchen alle 4 Kanten.
            if is_free_roam_obs:
                off_screen = obs.right < 0 or obs.left > WIDTH or obs.bottom < 0 or obs.top > HEIGHT
            else:
                off_screen = obs.right < 0
            if off_screen:
                obstacles.remove(obs)
                score += 1
                ctx["score"] = score
                check_achievements(ctx)
                # Mod hook: enemy passed (survived)
                _mod_sync_in()
                mod_system.emit('on_enemy_killed', _mod_game_ref, obs)
                _mod_sync_out()

        # Move top obstacles downward (freeze stoppt sie)
        if rp.freeze_remaining == 0:
            for obs in top_obstacles[:]:
                obs.y += fall_speed
                if wind_v_enabled:
                    obs.y += wind_v_strength * 4.5   # vertikaler Wind wirkt auf ihre Fallgeschwindigkeit
                if wind_h_enabled:
                    obs.x += wind_h_strength * 5.0    # horizontaler Wind lässt sie seitwärts driften
                if obs.top > HEIGHT:
                    top_obstacles.remove(obs)

        # Nuke destroys obstacles around player (including top obstacles)
        if rp.nuke_remaining > 0:
            for obs in obstacles[:]:
                dx = obs.centerx - player.centerx
                dy = obs.centery - player.centery
                if dx*dx + dy*dy < 40000:
                    obstacles.remove(obs)
            for obs in top_obstacles[:]:
                dx = obs.centerx - player.centerx
                dy = obs.centery - player.centery
                if dx*dx + dy*dy < 40000:
                    top_obstacles.remove(obs)

        for p in powerups[:]:
            p.x -= current_speed
            if rp.magnet:
                dx, dy = player.centerx - p.centerx, player.centery - p.centery
                dist = (dx * dx + dy * dy) ** 0.5
                if 0 < dist < 200:
                    pull = 7 * (1 - dist / 200)
                    p.x += int(dx / dist * pull)
                    p.y += int(dy / dist * pull)
            if p.right < 0:
                powerups.remove(p)

        # Pickup 1-UP
        for p in powerups[:]:
            if player.colliderect(p):
                lives += 1
                oneups_collected += 1
                stats["total_1ups"] += 1
                ctx["1ups"] += 1
                ctx["1ups_total"] += 1
                powerups.remove(p)
                play_sound(oneup_sound)
                show_1up_text_overlay(player_rect=player)
                push_overlay("1-UP eingesammelt", (0,150,0), pos=(player.centerx, player.top-20))
                check_achievements(ctx)

        # Collision
        for obs in obstacles[:]:
            if player.colliderect(obs):
                if rp.freeze_remaining > 0 or hit_invuln > 0:
                    continue  # Frozen / kurze Unverwundbarkeit nach Treffer
                # Shield/Ghost check
                if rp.on_hit_block():
                    push_overlay(get_text("protected"), PURPLE, pos=(player.centerx, player.top-20))
                elif rp.auto_dodge and random.random() < 0.05:
                    push_overlay("Auto Dodge!", (0, 200, 0), pos=(player.centerx, player.top-20))
                else:
                    lives -= 1
                    hit_invuln = 750   # 750 ms unverwundbar nach Treffer
                    ctx["no_damage"] = False
                    play_sound(hit_sound, "hit_volume")
                    # Mod hook: player took damage
                    _mod_sync_in()
                    mod_system.emit('on_player_damage', _mod_game_ref, 1)
                    _mod_sync_out()
                obstacles.remove(obs)
                if lives <= 0:
                    # Phoenix revival check
                    if rp.phoenix and rp.phoenix_cd == 0:
                        lives = 2
                        rp.phoenix_cd = 120000  # 2 minutes
                        rp.phoenix_invuln_remaining = 2000  # 2 seconds invulnerability
                        rp.phoenix_slow_remaining = 6000  # 6 seconds slow
                        flash_timer = 1000  # 1 second screen flash
                        push_overlay("Phoenix Revival!", (150, 50, 200), duration=2.0)
                    else:
                        end_time = time.time()
                        round_duration = end_time - round_start_time

                        # Stats
                        # Coin booster only affects coins; score remains real
                        final_score_display = score

                        # Coins calc
                        gained = compute_coins(score, difficulty, round_duration)
                        # Recovery: +3 coins if survived required time on hard+ difficulties
                        if "Recovery" in rp.equipped:
                            if difficulty in ["Schwer", "Hardcore", "Unmöglich", "Dynamisch"]:
                                required_time = 15 if difficulty == "Schwer" else 10
                                if round_duration >= required_time:
                                    gained += 3

                        round_seconds = int(round_duration)

                        if _mods_were_active:
                            # Modded run: save into separate modded storage
                            modded_stats["total_time_played"] = modded_stats.get("total_time_played", 0) + round_duration
                            modded_stats["total_rounds"]      = modded_stats.get("total_rounds", 0) + 1
                            modded_stats["coins"]             = modded_stats.get("coins", 0) + gained
                            save_modded_section("modded_stats", modded_stats)

                            old_highscore = modded_scores.get(difficulty, 0)
                            new_record = score > old_highscore
                            modded_scores[difficulty] = max(old_highscore, score)
                            modded_session_scores[difficulty] = max(modded_session_scores.get(difficulty, 0), score)
                            save_modded_section("modded_highscores", modded_scores)

                            modded_best_times[difficulty] = max(modded_best_times.get(difficulty, 0), round_seconds)
                            modded_session_times[difficulty] = max(modded_session_times.get(difficulty, 0), round_seconds)
                            save_modded_section("modded_best_times", modded_best_times)
                        else:
                            # Normal run: save into regular storage
                            stats["total_time_played"] += round_duration
                            stats["total_rounds"] += 1
                            stats["coins"] = stats.get("coins", 0) + gained
                            stats["total_deaths"] = stats.get("total_deaths", 0) + 1
                            stats["score_sum_by_diff"][difficulty] = stats["score_sum_by_diff"].get(difficulty, 0) + score
                            stats["rounds_by_diff"][difficulty] = stats["rounds_by_diff"].get(difficulty, 0) + 1
                            stats["total_coins_earned"] = stats.get("total_coins_earned", 0) + gained
                            save_section("stats", stats)

                            old_highscore = saved_scores.get(difficulty, 0)
                            new_record = score > old_highscore
                            saved_scores[difficulty] = max(old_highscore, score)
                            session_scores[difficulty] = max(session_scores.get(difficulty, 0), score)
                            save_section("highscores", saved_scores)
                            if not klassik_mode:
                                submit_score_online("standard", difficulty, score)

                            best_times[difficulty] = max(best_times.get(difficulty, 0), round_seconds)
                            session_times[difficulty] = max(session_times.get(difficulty, 0), round_seconds)
                            save_section("best_times", best_times)


                        # Achievements
                        ctx["hardcore_time"] = round_duration if difficulty == "Hardcore" else 0
                        ctx["impossible_time"] = round_duration if difficulty == "Unmöglich" else 0
                        ctx["nightmare_time"] = round_duration if difficulty == "Albtraum" else 0
                        ctx["hell_time"] = round_duration if difficulty == "Hölle" else 0
                        ctx["total_coins"] = stats.get("coins", 0) + (0 if _mods_were_active else gained)
                        check_achievements(ctx)

                        stop_music()
                        show_game_over(final_score_display, gained, old_highscore, new_record, round_seconds)
                        return

        # Collision with top obstacles
        for obs in top_obstacles[:]:
            if player.colliderect(obs):
                if rp.freeze_remaining > 0 or hit_invuln > 0:
                    continue  # Frozen / kurze Unverwundbarkeit nach Treffer
                if rp.on_hit_block():
                    push_overlay(get_text("protected"), PURPLE, pos=(player.centerx, player.top-20))
                elif rp.auto_dodge and random.random() < 0.05:
                    push_overlay("Auto Dodge!", (0, 200, 0), pos=(player.centerx, player.top-20))
                else:
                    lives -= 1
                    hit_invuln = 750   # 750 ms unverwundbar nach Treffer
                    ctx["no_damage"] = False
                    play_sound(hit_sound, "hit_volume")
                top_obstacles.remove(obs)
                if lives <= 0:
                    if rp.phoenix and rp.phoenix_cd == 0:
                        lives = 2
                        rp.phoenix_cd = 120000
                        rp.phoenix_invuln_remaining = 2000
                        rp.phoenix_slow_remaining = 6000
                        flash_timer = 1000
                        push_overlay("Phoenix Revival!", (150, 50, 200), duration=2.0)
                    else:
                        end_time = time.time()
                        round_duration = end_time - round_start_time
                        final_score_display = score
                        gained = compute_coins(score, difficulty, round_duration)
                        if "Recovery" in rp.equipped:
                            if difficulty in ["Schwer", "Hardcore", "Unmöglich", "Dynamisch"]:
                                required_time = 15 if difficulty == "Schwer" else 10
                                if round_duration >= required_time:
                                    gained += 3
                        round_seconds = int(round_duration)
                        if _mods_were_active:
                            modded_stats["total_time_played"] = modded_stats.get("total_time_played", 0) + round_duration
                            modded_stats["total_rounds"]      = modded_stats.get("total_rounds", 0) + 1
                            modded_stats["coins"]             = modded_stats.get("coins", 0) + gained
                            save_modded_section("modded_stats", modded_stats)
                            old_highscore = modded_scores.get(difficulty, 0)
                            new_record = score > old_highscore
                            modded_scores[difficulty] = max(old_highscore, score)
                            modded_session_scores[difficulty] = max(modded_session_scores.get(difficulty, 0), score)
                            save_modded_section("modded_highscores", modded_scores)
                            modded_best_times[difficulty] = max(modded_best_times.get(difficulty, 0), round_seconds)
                            modded_session_times[difficulty] = max(modded_session_times.get(difficulty, 0), round_seconds)
                            save_modded_section("modded_best_times", modded_best_times)
                        else:
                            stats["total_time_played"] += round_duration
                            stats["total_rounds"] += 1
                            stats["coins"] = stats.get("coins", 0) + gained
                            stats["total_deaths"] = stats.get("total_deaths", 0) + 1
                            stats["score_sum_by_diff"][difficulty] = stats["score_sum_by_diff"].get(difficulty, 0) + score
                            stats["rounds_by_diff"][difficulty] = stats["rounds_by_diff"].get(difficulty, 0) + 1
                            stats["total_coins_earned"] = stats.get("total_coins_earned", 0) + gained
                            save_section("stats", stats)
                            old_highscore = saved_scores.get(difficulty, 0)
                            new_record = score > old_highscore
                            saved_scores[difficulty] = max(old_highscore, score)
                            session_scores[difficulty] = max(session_scores.get(difficulty, 0), score)
                            save_section("highscores", saved_scores)
                            if not klassik_mode:
                                submit_score_online("standard", difficulty, score)
                            best_times[difficulty] = max(best_times.get(difficulty, 0), round_seconds)
                            session_times[difficulty] = max(session_times.get(difficulty, 0), round_seconds)
                            save_section("best_times", best_times)
                        ctx["hardcore_time"] = round_duration if difficulty == "Hardcore" else 0
                        ctx["impossible_time"] = round_duration if difficulty == "Unmöglich" else 0
                        ctx["nightmare_time"] = round_duration if difficulty == "Albtraum" else 0
                        ctx["hell_time"] = round_duration if difficulty == "Hölle" else 0
                        ctx["total_coins"] = stats.get("coins", 0) + (0 if _mods_were_active else gained)
                        check_achievements(ctx)
                        stop_music()
                        show_game_over(final_score_display, gained, old_highscore, new_record, round_seconds)
                        return

        # Dynamic mode evolves — no cap, keeps accelerating forever
        if dynamic_mode:
            dynamic_timer += dt
            if dynamic_timer >= dynamic_increase_interval:
                dynamic_timer = 0
                speed += dynamic_speed_increment
                min_delay = max(20, min_delay - 50)
                max_delay = max(min_delay + 20, max_delay - 100)
                ctx["speed_bonus"] = True
                check_achievements(ctx)

        # Update power timers
        rp.update_cooldowns(dt)
        rp.tick_shield_rearm()
        if hit_invuln > 0:
            hit_invuln = max(0, hit_invuln - dt)

        # Draw - Verbessertes Design
        # Player mit Schatten + Blink während hit_invuln
        _show_player = (hit_invuln == 0) or ((pygame.time.get_ticks() // 80) % 2 == 0)
        player_shadow = pygame.Rect(player.x + 3, player.y + 3, player.width, player.height)
        if _show_player:
            pygame.draw.rect(base_surface, (100, 100, 150), player_shadow, border_radius=8)
            pygame.draw.rect(base_surface, get_player_color(), player, border_radius=8)

        # Ausgerüstete Kopfbedeckung/Augen/Nase/Mund zeichnen (Kopfbedeckung, Augen, Nase
        # und Mund haben je einen eigenen Slot, daher gleichzeitig z.B. Zylinder +
        # Pixel-Augen + Clown-Nase + Breites Grinsen möglich) - siehe draw_player_cosmetics(),
        # dieselbe Funktion zeichnet auch die Avatar-Vorschau im Profil-Hub/Game-Over-Screen.
        draw_player_cosmetics(base_surface, player)

        # Obstacles mit Schatten
        for obs in obstacles:
            obs_shadow = pygame.Rect(obs.x + 2, obs.y + 2, obs.width, obs.height)
            pygame.draw.rect(base_surface, (150, 50, 50), obs_shadow, border_radius=6)
            pygame.draw.rect(base_surface, RED, obs, border_radius=6)

        # Top obstacles (fall from above) — lila mit Pfeil-Indikator
        for obs in top_obstacles:
            obs_shadow = pygame.Rect(obs.x + 2, obs.y + 2, obs.width, obs.height)
            pygame.draw.rect(base_surface, (80, 0, 110), obs_shadow, border_radius=6)
            pygame.draw.rect(base_surface, (160, 0, 200), obs, border_radius=6)
            # Kleiner Pfeil nach unten als visueller Hinweis
            ax = obs.centerx
            ay = obs.top - 8
            pygame.draw.polygon(base_surface, (200, 100, 255), [
                (ax, ay + 7), (ax - 6, ay), (ax + 6, ay)
            ])

        # Powerups mit Glow-Effekt
        for p in powerups:
            # Glow
            glow_rect = pygame.Rect(p.x - 3, p.y - 3, p.width + 6, p.height + 6)
            pygame.draw.rect(base_surface, (100, 255, 100), glow_rect, border_radius=10)
            pygame.draw.rect(base_surface, GREEN, p, border_radius=8)
            # 1-UP Symbol
            oneup_text = small_font.render("1-UP", True, WHITE)
            base_surface.blit(oneup_text, (p.centerx - oneup_text.get_width()//2, p.centery - oneup_text.get_height()//2))

        # Wind-Partikel (visueller Effekt für den Wind-Modifier, Celeste-artige Streifen).
        # Fester Pool, der beim Verlassen des Bildschirms irgendwo NEU auftaucht (nicht nur
        # an einer Kante nachspawnt) - dadurch ist der Effekt immer über den ganzen
        # Bildschirm verteilt, auch direkt nach einem Richtungswechsel.
        if wind_h_enabled or wind_v_enabled:
            if rp.freeze_remaining == 0:
                for wp in wind_particles:
                    wp[0] += wind_h_strength * 16
                    wp[1] += wind_v_strength * 16
                    if wp[0] < -60 or wp[0] > WIDTH + 60 or wp[1] < -60 or wp[1] > HEIGHT + 60:
                        wp[0] = random.randint(0, WIDTH)
                        wp[1] = random.randint(0, HEIGHT)
                        wp[2] = random.randint(30, 70)
            _wind_mag = max(abs(wind_h_strength), abs(wind_v_strength))
            if _wind_mag > 0.04:
                _wind_alpha = min(200, int(_wind_mag * 220))
                _dist = max(0.001, (wind_h_strength ** 2 + wind_v_strength ** 2) ** 0.5)
                _ux, _uy = wind_h_strength / _dist, wind_v_strength / _dist
                _wind_layer = pygame.Surface((WIDTH, HEIGHT), pygame.SRCALPHA)
                for wx, wy, wlen in wind_particles:
                    pygame.draw.line(_wind_layer, (220, 240, 255, _wind_alpha),
                                      (wx - _ux * wlen, wy - _uy * wlen), (wx, wy), 2)
                base_surface.blit(_wind_layer, (0, 0))

        # HUD - kein Box-Hintergrund, Text mit Schatten
        _lh = 22
        _tx = 12
        _ty = 10
        _tcol = (230, 235, 255) if is_dark_mode() else (20, 20, 50)
        _shcol = (0, 0, 0) if is_dark_mode() else (180, 185, 210)

        def _hud_text(line, x, y):
            sh = small_font.render(line, True, _shcol)
            s = small_font.render(line, True, _tcol)
            base_surface.blit(sh, (x + 1, y + 1))
            base_surface.blit(s, (x, y))

        _elapsed_s = int(time.time() - round_start_time)
        for _i, _line in enumerate([
            f"{get_text('score')}: {score}",
            f"{get_text('lives')}: {lives}",
            f"{get_text('highscore')}: {(modded_scores if _mods_were_active else saved_scores).get(difficulty, 0)}",
            f"{get_text('coins')}: {(modded_stats if _mods_were_active else stats).get('coins', 0)}",
        ]):
            _hud_text(_line, _tx, _ty + _i * _lh)
        _time_line = f"{_elapsed_s}s"
        _score_w = small_font.size(f"{get_text('score')}: {score}")[0]
        _hud_text(_time_line, _tx + _score_w + 14, _ty)

        if wind_h_enabled or wind_v_enabled:
            _h_dir = 1 if wind_h_strength > 0.05 else (-1 if wind_h_strength < -0.05 else 0)
            _v_dir = 1 if wind_v_strength > 0.05 else (-1 if wind_v_strength < -0.05 else 0)
            _wind_pct = int(max(abs(wind_h_strength), abs(wind_v_strength)) * 100)
            _wind_label = f"Wind: {_wind_pct}%"
            _wind_row_y = _ty + 4 * _lh
            _hud_text(_wind_label, _tx, _wind_row_y)
            # Richtungspfeil als Vektorgrafik statt Unicode-Zeichen (nicht jede Font hat →↑↓ etc.)
            _aw = small_font.size(_wind_label)[0]
            _acx = _tx + _aw + 16
            _acy = _wind_row_y + small_font.get_height() // 2
            if _h_dir == 0 and _v_dir == 0:
                pygame.draw.circle(base_surface, _tcol, (_acx, _acy), 3)
            else:
                _alen = max(0.001, (_h_dir ** 2 + _v_dir ** 2) ** 0.5)
                _aux, _auy = _h_dir / _alen, _v_dir / _alen
                _asz = 9
                _tip = (_acx + _aux * _asz, _acy + _auy * _asz)
                _tail = (_acx - _aux * _asz, _acy - _auy * _asz)
                _perp = (-_auy, _aux)
                _wing1 = (_tip[0] - _aux * _asz * 0.7 + _perp[0] * _asz * 0.5,
                          _tip[1] - _auy * _asz * 0.7 + _perp[1] * _asz * 0.5)
                _wing2 = (_tip[0] - _aux * _asz * 0.7 - _perp[0] * _asz * 0.5,
                          _tip[1] - _auy * _asz * 0.7 - _perp[1] * _asz * 0.5)
                pygame.draw.line(base_surface, _tcol, _tail, _tip, 2)
                pygame.draw.polygon(base_surface, _tcol, [_tip, _wing1, _wing2])


        # Power HUD (rechts oben)
        _py_screen = 10
        if rp.equipped:
            for name in rp.equipped:
                rar = POWERS_DB.get(name, {}).get("rarity", "?")
                color = rarity_colors.get(rar, GRAY)
                _icon = 16
                _ix = WIDTH - 220
                pygame.draw.rect(base_surface, color, pygame.Rect(_ix, _py_screen + 2, _icon, _icon), border_radius=3)
                label = get_power_name(name)
                if name in rp.timers and rp.timers[name] > 0:
                    label += f" ({rp.timers[name]//1000}s)"
                if name == 'Slow Motion' and rp.slow_remaining > 0:
                    label += f" [{rp.slow_remaining//1000}s]"
                if name == 'Time Freeze':
                    if rp.freeze_remaining > 0:
                        label += f" [Freeze {rp.freeze_remaining//1000}s]"
                    elif rp.freeze_slow_remaining > 0:
                        label += f" [Slow {rp.freeze_slow_remaining//1000}s]"
                if name == 'Ghost Mode' and rp.ghost_remaining > 0:
                    label += f" [{rp.ghost_remaining//1000}s]"
                if name == 'Nuke' and rp.nuke_remaining > 0:
                    label += f" [{rp.nuke_remaining//1000}s]"
                if name == 'Phoenix':
                    if rp.phoenix_invuln_remaining > 0:
                        label += f" [Invuln {rp.phoenix_invuln_remaining//1000}s]"
                    elif rp.phoenix_slow_remaining > 0:
                        label += f" [Slow {rp.phoenix_slow_remaining//1000}s]"
                    elif rp.phoenix_cd > 0:
                        label += f" [{rp.phoenix_cd//60000}:{(rp.phoenix_cd//1000 % 60):02d}]"
                _hud_text(label, _ix + _icon + 6, _py_screen)
                _py_screen += _lh

        # Bind hints for actives
        binds = rp.active_bindings()
        if len(binds) >= 1:
            _hud_text(f"Q: {binds[0]}", WIDTH - 220, _py_screen)
            _py_screen += _lh
        if len(binds) >= 2:
            _hud_text(f"E: {binds[1]}", WIDTH - 220, _py_screen)

        # draw overlays (top / per pos)
        cleanup_overlays()
        for ov in overlays:
            txt = small_font.render(ov["text"], True, ov["color"])
            if ov["pos"] is None:
                base_surface.blit(txt, ((WIDTH - txt.get_width()) // 2, 8))
            else:
                # pos provided relative to game world
                px, py = ov["pos"]
                # ensure it's on base_surface
                sx = max(8, min(WIDTH - txt.get_width() - 8, px - txt.get_width()//2))
                sy = max(8, min(HEIGHT - txt.get_height() - 8, py - txt.get_height()))
                base_surface.blit(txt, (sx, sy))
        draw_ingame_achievement_banner(base_surface)

        # Draw all trail particles with enhanced visual effects
        equipped_trails = [t for t in ALL_TRAIL_KEYS
                          if t in cosmetics_state.get("owned", []) and t in cosmetics_state.get("equipped", [])]

        for trail in equipped_trails:
            particles = trail_particle_lists[trail]

            for p in particles:
                progress = p['life'] / p['max_life']
                if progress <= 0: continue

                # Enhanced size calculation with trail-specific variations
                base_size = p.get('size', 2.5)
                size_multiplier = {
                    "fire_trail": 1.2,
                    "ice_trail": 1.0,
                    "shadow_trail": 0.8,
                    "sparkle_trail": 1.5,
                    "toxic_trail": 1.1,
                    "electric_trail": 0.9,
                    "rainbow_trail": 1.3,
                    "star_trail": 1.2,
                    "smoke_trail": 1.4,
                    "blood_trail": 1.1,
                    "heart_trail": 1.3
                }.get(trail, 1.0)

                radius = max(1, int(base_size * progress * size_multiplier))

                # Enhanced color calculations with better gradients (clamped to 0-255)
                def clamp_color(c):
                    return max(0, min(255, c))

                if trail == "fire_trail":
                    # Fire: dynamic yellow-orange-red with brightness variation
                    brightness = clamp_color(200 + int(55 * progress))
                    if progress > 0.8:
                        color = (brightness, brightness, 0)  # Bright yellow
                    elif progress > 0.5:
                        color = (brightness, clamp_color(int(brightness * 0.65)), 0)  # Orange
                    elif progress > 0.2:
                        color = (brightness, clamp_color(int(brightness * 0.3)), 0)   # Red-orange
                    else:
                        color = (brightness, 0, 0)  # Deep red

                elif trail == "ice_trail":
                    # Ice: blue-white gradient with shimmer effect
                    if progress > 0.7:
                        color = (220, 240, 255)  # Ice white
                    elif progress > 0.4:
                        color = (150, 200, 255)  # Light blue
                    else:
                        color = (80, 160, 255)   # Deep ice blue

                elif trail == "shadow_trail":
                    # Shadow: smooth black to dark gray with fade
                    intensity = clamp_color(int(120 * progress))
                    color = (intensity, intensity, intensity)

                elif trail == "sparkle_trail":
                    # Sparkle: golden sparkle with twinkle effect
                    twinkle = 0.8 + 0.4 * (1 if int(p.get('angle', 0) * progress) % 3 == 0 else 0)
                    brightness = clamp_color(int(255 * progress * twinkle))
                    if progress > 0.6:
                        color = (brightness, brightness, clamp_color(int(brightness * 0.8)))  # White-gold
                    elif progress > 0.3:
                        color = (brightness, clamp_color(int(brightness * 0.9)), clamp_color(int(brightness * 0.3)))  # Gold
                    else:
                        color = (clamp_color(int(brightness * 0.8)), clamp_color(int(brightness * 0.7)), 0)  # Yellow-gold

                elif trail == "toxic_trail":
                    # Toxic: green-purple gradient with eerie glow
                    if progress > 0.7:
                        color = (150, 220, 100)  # Bright toxic green
                    elif progress > 0.4:
                        color = (120, 180, 150)  # Green-purple mix
                    else:
                        color = (100, 80, 180)   # Deep purple

                elif trail == "electric_trail":
                    # Electric: white-hot core fading to deep blue
                    if progress > 0.7:
                        color = (225, 250, 255)  # Near-white spark
                    elif progress > 0.4:
                        color = (120, 220, 255)  # Bright cyan-blue
                    else:
                        color = (40, 130, 220)   # Fading deep blue

                elif trail == "rainbow_trail":
                    # Rainbow: rotierender Regenbogen-Farbverlauf (HSV -> RGB) je Partikel,
                    # angetrieben vom 'angle'-Wert, der bei der Physik-Aktualisierung dreht
                    hue = (p.get('angle', 0) / 360.0) % 1.0
                    r, g, b = colorsys.hsv_to_rgb(hue, 0.85, 0.55 + 0.45 * progress)
                    color = (clamp_color(int(r * 255)), clamp_color(int(g * 255)), clamp_color(int(b * 255)))

                elif trail == "star_trail":
                    # Star: kühles Weiß-Blau mit Funkeln
                    twinkle = 0.75 + 0.35 * (1 if int(p.get('angle', 0) * progress) % 3 == 0 else 0)
                    brightness = clamp_color(int(255 * progress * twinkle))
                    if progress > 0.6:
                        color = (brightness, brightness, brightness)  # Reines Weiß
                    else:
                        color = (clamp_color(int(brightness * 0.8)), clamp_color(int(brightness * 0.9)), brightness)  # Eisblau

                elif trail == "smoke_trail":
                    # Smoke: grauer Rauch, der beim Verblassen heller/durchsichtiger wirkt
                    grey = clamp_color(int(90 + 100 * progress))
                    color = (grey, grey, clamp_color(grey + 5))

                elif trail == "blood_trail":
                    # Blood: dunkles, sattes Rot, das beim Verblassen fast schwarz wird
                    if progress > 0.6:
                        color = (200, 20, 30)   # Frisches Blutrot
                    elif progress > 0.3:
                        color = (150, 10, 20)   # Dunkleres Rot
                    else:
                        color = (80, 5, 10)     # Fast schwarz

                elif trail == "heart_trail":
                    # Heart: pinkes bis kräftig rotes Herz-Glühen
                    if progress > 0.6:
                        color = (255, 180, 200)  # Zartes Pink
                    elif progress > 0.3:
                        color = (230, 40, 90)    # Kräftiges Pink-Rot
                    else:
                        color = (160, 10, 50)    # Dunkles Rot

                # Draw main particle
                pygame.draw.circle(base_surface, color, (int(p['x']), int(p['y'])), radius)

                # Add special effects for certain trails
                if trail == "ice_trail" and progress > 0.5:
                    # Ice particles get a subtle glow
                    glow_radius = radius + 1
                    glow_color = (int(color[0] * 0.7), int(color[1] * 0.7), int(color[2] * 0.7))
                    pygame.draw.circle(base_surface, glow_color, (int(p['x']), int(p['y'])), glow_radius)

                elif trail == "sparkle_trail":
                    # Sparkles get star-like points
                    if radius > 2:
                        for angle in range(0, 360, 45):  # 8-point star
                            sparkle_x = int(p['x'] + (radius + 2) * 0.7 * (1 - progress) * (angle / 180.0))
                            sparkle_y = int(p['y'])
                            pygame.draw.circle(base_surface, (255, 255, 200), (sparkle_x, sparkle_y), 1)

                elif trail == "toxic_trail" and progress > 0.3:
                    # Toxic particles get a subtle outer glow
                    glow_radius = max(2, radius + 1)
                    glow_alpha = int(100 * progress)
                    glow_color = (int(color[0] * 0.5), int(color[1] * 0.5), int(color[2] * 0.5))
                    pygame.draw.circle(base_surface, glow_color, (int(p['x']), int(p['y'])), glow_radius)

                elif trail == "electric_trail" and progress > 0.4:
                    # Sparks get a short flickering jolt line
                    jx = int(p['x'] + random.uniform(-4, 4))
                    jy = int(p['y'] + random.uniform(-4, 4))
                    pygame.draw.line(base_surface, (200, 240, 255), (int(p['x']), int(p['y'])), (jx, jy), 1)

                elif trail == "rainbow_trail" and radius > 1:
                    # Regenbogenspur bekommt einen leichten weißen Glanzkern
                    pygame.draw.circle(base_surface, (255, 255, 255), (int(p['x']), int(p['y'])), max(1, radius - 1), width=1)

                elif trail == "star_trail" and radius > 1:
                    # Sterne bekommen kleine funkelnde Zacken
                    for _sa in (0, 90, 180, 270):
                        _srad = math.radians(_sa + p.get('angle', 0))
                        sx2 = int(p['x'] + math.cos(_srad) * (radius + 3) * progress)
                        sy2 = int(p['y'] + math.sin(_srad) * (radius + 3) * progress)
                        pygame.draw.line(base_surface, (255, 255, 255), (int(p['x']), int(p['y'])), (sx2, sy2), 1)

                elif trail == "smoke_trail" and progress > 0.2:
                    # Rauchpartikel bekommen einen weichen, durchscheinenden Rand
                    glow_radius = radius + 2
                    glow_color = (clamp_color(color[0] + 15), clamp_color(color[1] + 15), clamp_color(color[2] + 15))
                    pygame.draw.circle(base_surface, glow_color, (int(p['x']), int(p['y'])), glow_radius, width=1)

                elif trail == "blood_trail" and progress > 0.4:
                    # Blutstropfen bekommen eine kleine "Schweif"-Linie nach oben (Fallrichtung)
                    tx2 = int(p['x'] - p.get('vx', 0) * 2)
                    ty2 = int(p['y'] - p.get('vy', 0) * 2)
                    pygame.draw.line(base_surface, color, (int(p['x']), int(p['y'])), (tx2, ty2), max(1, radius - 1))

                elif trail == "heart_trail" and radius > 1:
                    # Herzen bekommen einen zarten hellen Kern
                    pygame.draw.circle(base_surface, (255, 220, 230), (int(p['x']), int(p['y'])), max(1, radius - 2))


        # Screen flash effect
        if flash_timer > 0:
            flash_surface = pygame.Surface((WIDTH, HEIGHT))
            alpha = min(255, int(flash_timer / 10))
            flash_surface.set_alpha(alpha)
            flash_surface.fill((255, 255, 255))
            base_surface.blit(flash_surface, (0, 0))

        # Almost Dead: red vignette (top only) + heartbeat sound when lives == 1
        if lives == 1:
            almost_dead_pulse += dt
            heartbeat_timer += dt
            if heartbeat_timer >= heartbeat_interval:
                heartbeat_timer = 0
                play_sound(heartbeat_sound, "sfx_volume")

            # Pulse: 0.3 – 0.85 so it stays readable
            pulse = 0.3 + 0.55 * abs(sin(almost_dead_pulse * pi / heartbeat_interval))

            layers = 30
            vig_top  = HEIGHT // 3
            vig_side = WIDTH  // 5

            # Three separate surfaces blitted independently so corners
            # receive combined alpha from all three passes
            def _make_vig(w, h):
                return pygame.Surface((w, h), pygame.SRCALPHA)

            # Top gradient
            top_s = _make_vig(WIDTH, vig_top)
            for li in range(layers):
                frac    = (layers - li) / layers
                alpha_v = int(90 * pulse * frac ** 1.6)
                strip_h = max(1, vig_top // layers + 2)
                pygame.draw.rect(top_s, (210, 0, 0, alpha_v),
                                 (0, li * (vig_top // layers), WIDTH, strip_h))
            base_surface.blit(top_s, (0, 0))

            # Left gradient
            left_s = _make_vig(vig_side, HEIGHT)
            for li in range(layers):
                frac    = (layers - li) / layers
                alpha_v = int(90 * pulse * frac ** 1.6)
                strip_w = max(1, vig_side // layers + 2)
                pygame.draw.rect(left_s, (210, 0, 0, alpha_v),
                                 (li * (vig_side // layers), 0, strip_w, HEIGHT))
            base_surface.blit(left_s, (0, 0))

            # Right gradient — flip left_s so the strong edge faces outward
            right_s = pygame.transform.flip(left_s, True, False)
            base_surface.blit(right_s, (WIDTH - vig_side, 0))
        else:
            almost_dead_pulse = 0.0
            heartbeat_timer = 0

        _last_game_frame = base_surface.copy()
        pygame.display.flip()


# =============================
# 2-Player Game Mode
# =============================

def run_game_2p():
    """Same-screen 2-player game.
    P1: WASD + Space   (blau)
    P2: Pfeiltasten    (orange)
    Co-op: beide überleben gemeinsam; VS: letzter Überlebender gewinnt.
    """
    global game_running, in_menu
    clock = pygame.time.Clock()
    difficulty = difficulties[difficulty_index]
    speed      = difficulty_speeds[difficulty]
    play_ingame_music()
    min_delay, max_delay = difficulty_obstacle_intervals[difficulty]

    dynamic_mode              = (difficulty == "Dynamisch")
    dynamic_timer             = 0
    dynamic_increase_interval = 5000
    dynamic_speed_increment   = 0.5

    floor_y    = HEIGHT - 50
    move_speed = 8
    gravity    = 1

    # --- Beide Spieler ---
    p1 = pygame.Rect(60,  floor_y - 40, 40, 40)   # blau, links
    p2 = pygame.Rect(140, floor_y - 40, 40, 40)   # orange, daneben

    vy1 = 0;    vy2 = 0
    jump1 = False; jump2 = False
    up1_was = False; up2_was = False
    alive1 = True;  alive2 = True
    lives1 = 3;     lives2 = 3
    invuln1 = 0;    invuln2 = 0    # Unverwundbarkeit in Frames nach Treffer

    # --- Hindernisse & Score ---
    obstacles     = []
    top_obstacles = []
    powerups      = []
    obs_timer     = 0
    obstacle_delay_bag = ObstacleDelayBag()
    obs_delay     = obstacle_delay_bag.next(min_delay, max_delay)
    top_obs_timer = 0
    top_obs_delay = int(random.randint(900, 1600) * (800 / max(400, WIDTH)))
    score         = 0

    vs_mode    = (two_player_submode == "vs")
    round_start = time.time()

    # HUD-Helfer: kleine Lebensquadrate zeichnen
    def _draw_lives_left(x, y, lives, color):
        """Lebensquadrate links ausgerichtet ab x,y."""
        for i in range(max(3, lives)):
            col = color if i < lives else (55, 55, 55)
            pygame.draw.rect(base_surface, col,
                             pygame.Rect(x + i * 15, y, 11, 11), border_radius=3)

    def _draw_lives_right(right_x, y, lives, color):
        """Lebensquadrate rechts ausgerichtet, endet bei right_x."""
        count = max(3, lives)
        for i in range(count):
            col = color if i < lives else (55, 55, 55)
            dx = right_x - (count - i) * 15
            pygame.draw.rect(base_surface, col,
                             pygame.Rect(dx, y, 11, 11), border_radius=3)

    _last_game_frame_2p = base_surface.copy()

    while True:
        sync_window_size()

        # Siehe run_game(): game_running wird normalerweise nie extern False gesetzt,
        # ohne dass der jeweilige Code-Pfad sofort selbst zurückkehrt - falls doch,
        # kam das von aussen (Dev-Konsole) und die Runde wird sauber verlassen.
        if not game_running:
            stop_music()
            return

        dt = clock.tick(60)
        fill_bg(rings=True)

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN and action_matches_key(event.key, "pause"):
                _pause_t = time.time()
                _pause_result = _run_pause_menu(frozen=_last_game_frame_2p)
                round_start += time.time() - _pause_t
                clock.tick()  # accumulated dt verwerfen
                if _pause_result == "menu":
                    stop_music(); game_running = False; in_menu = True; return
                elif _pause_result == "restart":
                    stop_music(); return
                # "resume": continue

        keys = pygame.key.get_pressed()

        # ---- P1: rebindbare Steuerung (Standard: SPACE/W = Sprung, A/D = Bewegung) ----
        if alive1:
            p1_jump = any_action_pressed(keys, "p1_jump")
            if p1_jump and not up1_was:
                if not jump1:
                    vy1 = -20
                    jump1 = True
                    stats["total_jumps"] = stats.get("total_jumps", 0) + 1
            up1_was = p1_jump
            if any_action_pressed(keys, "p1_left"):  p1.x -= move_speed
            if any_action_pressed(keys, "p1_right"): p1.x += move_speed
            p1.left  = max(0, p1.left)
            p1.right = min(WIDTH, p1.right)
            vy1 += gravity
            p1.y += vy1
            if p1.bottom >= floor_y:
                p1.bottom = floor_y
                vy1 = 0
                if jump1:
                    jump1 = False
                    up1_was = False

        # ---- P2: rebindbare Steuerung (Standard: Pfeiltasten) ----
        if alive2:
            p2_jump = any_action_pressed(keys, "p2_jump")
            if p2_jump and not up2_was:
                if not jump2:
                    vy2 = -20
                    jump2 = True
                    stats["total_jumps"] = stats.get("total_jumps", 0) + 1
            up2_was = p2_jump
            if any_action_pressed(keys, "p2_left"):  p2.x -= move_speed
            if any_action_pressed(keys, "p2_right"): p2.x += move_speed
            p2.left  = max(0, p2.left)
            p2.right = min(WIDTH, p2.right)
            vy2 += gravity
            p2.y += vy2
            if p2.bottom >= floor_y:
                p2.bottom = floor_y
                vy2 = 0
                if jump2:
                    jump2 = False
                    up2_was = False

        if invuln1 > 0: invuln1 -= 1
        if invuln2 > 0: invuln2 -= 1

        # ---- Hindernisse spawnen ----
        obs_timer += dt
        if obs_timer >= obs_delay:
            obs_timer = 0
            obs_delay = obstacle_delay_bag.next(min_delay, max_delay)
            if random.random() < 0.015:
                powerups.append(pygame.Rect(WIDTH + 50, floor_y - 50, 38, 38))
            else:
                obstacles.extend(make_obstacle(WIDTH + 50, pick_obstacle_type(difficulty, speed if dynamic_mode else None)))

        _top_active = difficulty in {"Hardcore", "Unmöglich", "Albtraum", "Hölle"} or (dynamic_mode and speed >= 14)
        if _top_active:
            top_obs_timer += dt
            if top_obs_timer >= top_obs_delay:
                top_obs_timer = 0
                top_obstacles.append(make_top_obstacle())
                top_obs_delay = int(random.randint(900, 1600) * (800 / max(400, WIDTH)))

        current_speed = speed
        fall_speed    = max(2.0, current_speed * 0.85)

        # ---- Hindernisse bewegen ----
        for obs in obstacles[:]:
            obs.x -= current_speed
            if obs.right < 0:
                obstacles.remove(obs)
                score += 1
        for obs in top_obstacles[:]:
            obs.y += fall_speed
            if obs.top > HEIGHT:
                top_obstacles.remove(obs)
        for p in powerups[:]:
            p.x -= current_speed
            if p.right < 0:
                powerups.remove(p)

        # ---- 1-UP Pickup ----
        # Kürze Unverwundbarkeit nach Wiederbelebung (halbe Hit-Invuln)
        REVIVE_INVULN = 45
        for p in powerups[:]:
            picked = False
            if alive1 and p1.colliderect(p):
                if not alive2:
                    # P2 wiederbeleben
                    alive2 = True
                    lives2 = 1
                    p2.x   = 140; p2.bottom = floor_y
                    vy2 = 0; jump2 = False; up2_was = False
                    invuln2 = REVIVE_INVULN
                    push_overlay("P2 wiederbelebt!", (255, 160, 50),
                                 pos=(p2.centerx, p2.top - 20))
                else:
                    lives1 += 1
                    push_overlay("1-UP! (P1)", (0, 200, 0),
                                 pos=(p1.centerx, p1.top - 20))
                powerups.remove(p); picked = True
            elif alive2 and p2.colliderect(p):
                if not alive1:
                    # P1 wiederbeleben
                    alive1 = True
                    lives1 = 1
                    p1.x   = 60; p1.bottom = floor_y
                    vy1 = 0; jump1 = False; up1_was = False
                    invuln1 = REVIVE_INVULN
                    push_overlay("P1 wiederbelebt!", (100, 150, 255),
                                 pos=(p1.centerx, p1.top - 20))
                else:
                    lives2 += 1
                    push_overlay("1-UP! (P2)", (0, 200, 0),
                                 pos=(p2.centerx, p2.top - 20))
                powerups.remove(p); picked = True
            if picked:
                break

        # ---- Kollision P1 ----
        just_died1 = False
        just_died2 = False

        for obs in obstacles[:]:
            if alive1 and invuln1 == 0 and p1.colliderect(obs):
                lives1 -= 1
                play_sound(hit_sound, "hit_volume")
                obstacles.remove(obs)
                if lives1 <= 0:
                    alive1 = False
                    just_died1 = True
                else:
                    invuln1 = 90
                    push_overlay("P1 Treffer!", (100, 150, 255), pos=(p1.centerx, p1.top - 20))
                break

        for obs in top_obstacles[:]:
            if alive1 and invuln1 == 0 and p1.colliderect(obs):
                lives1 -= 1
                play_sound(hit_sound, "hit_volume")
                top_obstacles.remove(obs)
                if lives1 <= 0:
                    alive1 = False
                    just_died1 = True
                else:
                    invuln1 = 90
                    push_overlay("P1 Treffer!", (100, 150, 255), pos=(p1.centerx, p1.top - 20))
                break

        # ---- Kollision P2 ----
        for obs in obstacles[:]:
            if alive2 and invuln2 == 0 and p2.colliderect(obs):
                lives2 -= 1
                play_sound(hit_sound, "hit_volume")
                obstacles.remove(obs)
                if lives2 <= 0:
                    alive2 = False
                    just_died2 = True
                else:
                    invuln2 = 90
                    push_overlay("P2 Treffer!", (255, 160, 50), pos=(p2.centerx, p2.top - 20))
                break

        for obs in top_obstacles[:]:
            if alive2 and invuln2 == 0 and p2.colliderect(obs):
                lives2 -= 1
                play_sound(hit_sound, "hit_volume")
                top_obstacles.remove(obs)
                if lives2 <= 0:
                    alive2 = False
                    just_died2 = True
                else:
                    invuln2 = 90
                    push_overlay("P2 Treffer!", (255, 160, 50), pos=(p2.centerx, p2.top - 20))
                break

        # ---- Dynamic mode ----
        if dynamic_mode:
            dynamic_timer += dt
            if dynamic_timer >= dynamic_increase_interval:
                dynamic_timer = 0
                speed += dynamic_speed_increment
                min_delay = max(20, min_delay - 50)
                max_delay = max(min_delay + 20, max_delay - 100)

        # ---- Game-Over-Check ----
        game_over = False
        winner    = None
        if vs_mode:
            if just_died1 and just_died2:
                winner = "draw"; game_over = True
            elif just_died1:
                winner = "p2";   game_over = True
            elif just_died2:
                winner = "p1";   game_over = True
        else:  # co-op: beide müssen sterben
            if not alive1 and not alive2:
                game_over = True

        # ====== RENDERING ======

        # Seiten-Hindernisse
        for obs in obstacles:
            pygame.draw.rect(base_surface, (150, 50, 50),
                             pygame.Rect(obs.x + 2, obs.y + 2, obs.width, obs.height),
                             border_radius=6)
            pygame.draw.rect(base_surface, RED, obs, border_radius=6)

        # Oben-Hindernisse
        for obs in top_obstacles:
            pygame.draw.rect(base_surface, (80, 0, 110),
                             pygame.Rect(obs.x + 2, obs.y + 2, obs.width, obs.height),
                             border_radius=6)
            pygame.draw.rect(base_surface, (160, 0, 200), obs, border_radius=6)
            ax, ay = obs.centerx, obs.top - 8
            pygame.draw.polygon(base_surface, (200, 100, 255),
                                [(ax, ay + 7), (ax - 6, ay), (ax + 6, ay)])

        # Powerups
        for p in powerups:
            pygame.draw.rect(base_surface, (100, 255, 100),
                             pygame.Rect(p.x - 3, p.y - 3, p.width + 6, p.height + 6),
                             border_radius=10)
            pygame.draw.rect(base_surface, GREEN, p, border_radius=8)
            t = small_font.render("1-UP", True, WHITE)
            base_surface.blit(t, (p.centerx - t.get_width() // 2,
                                  p.centery - t.get_height() // 2))

        # Spieler 1 (blau) — blinkt während Unverwundbarkeit
        if alive1:
            show_p1 = (invuln1 == 0) or ((invuln1 // 5) % 2 == 0)
            if show_p1:
                pygame.draw.rect(base_surface, (70, 70, 130),
                                 pygame.Rect(p1.x + 3, p1.y + 3, p1.width, p1.height),
                                 border_radius=8)
                pygame.draw.rect(base_surface, BLUE, p1, border_radius=8)
            lbl1 = small_font.render("P1", True, WHITE)
            lbl1_sh = small_font.render("P1", True, (0, 0, 0))
            _lx1 = p1.centerx - lbl1.get_width() // 2
            _ly1 = p1.top - 16
            base_surface.blit(lbl1_sh, (_lx1 + 1, _ly1 + 1))
            base_surface.blit(lbl1, (_lx1, _ly1))

        # Spieler 2 (orange) — blinkt während Unverwundbarkeit
        if alive2:
            show_p2 = (invuln2 == 0) or ((invuln2 // 5) % 2 == 0)
            if show_p2:
                pygame.draw.rect(base_surface, (130, 70, 30),
                                 pygame.Rect(p2.x + 3, p2.y + 3, p2.width, p2.height),
                                 border_radius=8)
                pygame.draw.rect(base_surface, (255, 120, 0), p2, border_radius=8)
            lbl2 = small_font.render("P2", True, WHITE)
            lbl2_sh = small_font.render("P2", True, (0, 0, 0))
            _lx2 = p2.centerx - lbl2.get_width() // 2
            _ly2 = p2.top - 16
            base_surface.blit(lbl2_sh, (_lx2 + 1, _ly2 + 1))
            base_surface.blit(lbl2, (_lx2, _ly2))

        # HUD — P1 links
        p1_col = BLUE if alive1 else (70, 70, 70)
        p1_lbl = menu_font.render("P1", True, p1_col)
        base_surface.blit(p1_lbl, (12, 10))
        _draw_lives_left(12, 10 + p1_lbl.get_height() + 3, lives1, p1_col)

        # HUD — P2 rechts
        p2_col = (255, 120, 0) if alive2 else (70, 70, 70)
        p2_lbl = menu_font.render("P2", True, p2_col)
        base_surface.blit(p2_lbl, (WIDTH - 12 - p2_lbl.get_width(), 10))
        _draw_lives_right(WIDTH - 12, 10 + p2_lbl.get_height() + 3, lives2, p2_col)

        # HUD — Score + Zeit + Modus (zentriert oben)
        elapsed_s  = int(time.time() - round_start)
        mode_tag   = get_text("vs_mode_label") if vs_mode else get_text("coop_mode_label")
        score_surf = menu_font.render(f"{get_text('score')}: {score}", True, get_text_color())
        info_surf  = small_font.render(f"{mode_tag}  ·  {elapsed_s}s  ·  {get_difficulty_name(difficulty)}", True, (140, 60, 200))
        base_surface.blit(score_surf, (WIDTH // 2 - score_surf.get_width() // 2, 10))
        base_surface.blit(info_surf,  (WIDTH // 2 - info_surf.get_width()  // 2,
                                       10 + score_surf.get_height() + 2))

        # Nav-Hinweis (unten mitte, halbtransparent) — Tastenbelegung selbst steht
        # jetzt in Einstellungen > Steuerung und ist dort änderbar.
        hint = small_font.render(get_text("esc_menu_hint"), True, (90, 90, 110))
        base_surface.blit(hint, (WIDTH // 2 - hint.get_width() // 2, HEIGHT - 18))

        # "ELIMINATED"-Banner für tote Spieler
        if not alive1:
            elim = menu_font.render("P1 ELIMINATED", True, (200, 70, 70))
            base_surface.blit(elim, (20, HEIGHT // 2 - elim.get_height() // 2))
        if not alive2:
            elim = menu_font.render("P2 ELIMINATED", True, (200, 70, 70))
            base_surface.blit(elim, (WIDTH - elim.get_width() - 20,
                                     HEIGHT // 2 - elim.get_height() // 2))

        # Overlays (push_overlay Texte)
        cleanup_overlays()
        for ov in overlays:
            txt = small_font.render(ov["text"], True, ov["color"])
            if ov["pos"] is None:
                base_surface.blit(txt, ((WIDTH - txt.get_width()) // 2, 55))
            else:
                px, py = ov["pos"]
                sx = max(8, min(WIDTH - txt.get_width() - 8, px - txt.get_width() // 2))
                sy = max(8, min(HEIGHT - txt.get_height() - 8, py - txt.get_height()))
                base_surface.blit(txt, (sx, sy))

        _last_game_frame_2p = base_surface.copy()
        pygame.display.flip()

        if game_over:
            round_secs = int(time.time() - round_start)
            stop_music()
            stats["total_rounds"] = stats.get("total_rounds", 0) + 1
            stats["total_deaths"] = stats.get("total_deaths", 0) + 1
            stats["score_sum_by_diff"][difficulty] = stats["score_sum_by_diff"].get(difficulty, 0) + score
            stats["rounds_by_diff"][difficulty] = stats["rounds_by_diff"].get(difficulty, 0) + 1
            save_section("stats", stats)
            show_game_over_2p(score, winner, round_secs)
            return


def show_game_over_2p(score, winner, survived_seconds):
    """2-Player Game-Over-Screen mit Sieger-Anzeige und Rematch-Option."""
    global game_running, in_menu
    play_sound(gameover_sound, "gameover_volume")

    while True:
        fill_bg(rings=True)
        cx = WIDTH // 2

        # Titel je nach Ergebnis
        if winner == "p1":
            title_text  = "Spieler 1 gewinnt!"
            title_color = BLUE
        elif winner == "p2":
            title_text  = "Spieler 2 gewinnt!"
            title_color = (255, 120, 0)
        elif winner == "draw":
            title_text  = "Unentschieden!"
            title_color = (220, 200, 0)
        else:
            title_text  = "Game Over!"
            title_color = get_text_color()

        # Card-Panel hinter dem Inhalt (gleiche Behandlung wie beim 1-Player Game Over),
        # Rahmenfarbe folgt der Sieger-Farbe statt eines festen pulsierenden Golds -
        # hier gibt es keinen "neuer Highscore"-Moment, aber der Sieger ist die Nachricht.
        panel = pygame.Rect(cx - 240, HEIGHT // 2 - 200, 480, 320)
        draw_card(base_surface, panel, radius=20, border_color=title_color)

        title_surf = font.render(title_text, True, title_color)
        # Schatten
        title_sh   = font.render(title_text, True, (0, 0, 0))
        base_surface.blit(title_sh,   (cx - title_surf.get_width() // 2 + 3, HEIGHT // 2 - 148))
        base_surface.blit(title_surf, (cx - title_surf.get_width() // 2,     HEIGHT // 2 - 150))

        # Score-Zeile
        sc_surf = menu_font.render(f"Score: {score}   ·   {survived_seconds}s", True, get_text_color())
        base_surface.blit(sc_surf, (cx - sc_surf.get_width() // 2, HEIGHT // 2 - 75))

        # Modus-Info
        mode_str = get_text("vs_mode_label") if two_player_submode == "vs" else get_text("coop_mode_label")
        diff_str = difficulties[difficulty_index]
        sub_surf = small_font.render(f"{mode_str}  ·  {get_difficulty_name(diff_str)}", True, (140, 60, 200))
        base_surface.blit(sub_surf, (cx - sub_surf.get_width() // 2, HEIGHT // 2 - 45))

        # Buttons: Menü   Rematch
        btn_y        = HEIGHT // 2 + 20
        menu_rect    = pygame.Rect(cx - 215, btn_y, 195, 46)
        rematch_rect = pygame.Rect(cx + 20,  btn_y, 195, 46)
        _go2_rects   = [menu_rect, rematch_rect]

        mp = get_mouse_pos_logical()
        menu_hl    = menu_rect.collidepoint(mp)    or nav_is_focused("game_over_2p", 0, len(_go2_rects), default_idx=1)
        rematch_hl = rematch_rect.collidepoint(mp) or nav_is_focused("game_over_2p", 1, len(_go2_rects), default_idx=1)
        draw_ui_button(base_surface, menu_rect, get_text("back"), font=menu_font, hovered=menu_hl)
        draw_ui_button(base_surface, rematch_rect, "Rematch ↺", font=menu_font, hovered=rematch_hl,
                        base_color=(0, 130, 60), hover_color=(30, 175, 90), text_color=(255, 255, 255),
                        border_color=(60, 90, 70), hover_border_color=(190, 255, 210), hover_border_width=3)

        pygame.display.flip()

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if is_back_key(event.key):
                    game_running = False
                    in_menu = True
                    return
                d = nav_direction(event.key)
                if d:
                    cur = nav_get_focus("game_over_2p", len(_go2_rects), default_idx=1)
                    nav_set_focus("game_over_2p", nav_move(_go2_rects, cur, d))
                elif is_nav_confirm(event.key):
                    i = nav_get_focus("game_over_2p", len(_go2_rects), default_idx=1)
                    if i == 0:
                        game_running = False
                        in_menu = True
                        return
                    elif i == 1:
                        game_running = True
                        return
            if event.type == pygame.MOUSEBUTTONDOWN:
                mp2 = get_mouse_pos_logical()
                if menu_rect.collidepoint(mp2):
                    play_sound(click_back_sound)
                    game_running = False
                    in_menu = True
                    return
                if rematch_rect.collidepoint(mp2):
                    play_sound(menu_click_sound)
                    game_running = True
                    return


# =============================
# (Story Mode removed)
# =============================

# =============================
# Game Over
# =============================

def show_game_over(score_display, coins_gained, old_highscore=0, new_record=False, survived_seconds=0):
    global in_menu, game_running
    play_sound(gameover_sound, "gameover_volume")
    new_hs = max(old_highscore, score_display)
    while True:
        fill_bg(rings=True)
        cx = WIDTH // 2

        # Card-Panel hinter dem gesamten Inhalt (vorher: nackter Text direkt auf dem
        # animierten Hintergrund, im Gegensatz zu z.B. draw_results_overlay). Bei neuem
        # Highscore pulsierender Gold-Rahmen statt festem Rahmen.
        panel = pygame.Rect(cx - 240, HEIGHT // 2 - 300, 480, 460)
        draw_card(base_surface, panel, radius=20, glow=new_record,
                  glow_color=(255, 215, 0) if new_record else None)

        # Avatar + Profilname (Block in Wunschfarbe + ausgerüstete Kosmetik, siehe draw_avatar)
        _go_avatar_size = 50
        _go_avatar_rect = pygame.Rect(cx - _go_avatar_size // 2, HEIGHT // 2 - 265, _go_avatar_size, _go_avatar_size)
        draw_avatar(base_surface, _go_avatar_rect)
        _go_name_surf = small_font.render(current_profile_name or "-", True, get_text_color())
        base_surface.blit(_go_name_surf, (cx - _go_name_surf.get_width() // 2, _go_avatar_rect.bottom + 4))

        # Title mit Schatten (gleicher Stil wie Settings/Shop-Titel)
        over_s = font.render(get_text("game_over"), True, get_text_color())
        over_shadow = font.render(get_text("game_over"), True, (60, 60, 75) if is_dark_mode() else (170, 170, 190))
        base_surface.blit(over_shadow, (cx - over_s.get_width() // 2 + 2, HEIGHT // 2 - 168))
        base_surface.blit(over_s, (cx - over_s.get_width() // 2, HEIGHT // 2 - 170))

        # Score + Zeit nebeneinander
        score_label = menu_font.render(f"{get_text('score')}:  {score_display}", True, get_text_color())
        time_label  = menu_font.render(f"{survived_seconds}s", True, (140, 180, 220))
        total_w = score_label.get_width() + 20 + time_label.get_width()
        base_surface.blit(score_label, (cx - total_w // 2, HEIGHT // 2 - 100))
        base_surface.blit(time_label,  (cx - total_w // 2 + score_label.get_width() + 20, HEIGHT // 2 - 100))

        # Highscore section - neuer Rekord pulsiert (gleicher Glow-Effekt wie der Panel-Rahmen)
        if new_record:
            hs_col = ui_glow_pulse((255, 215, 0), speed=4.0, low=0.75)
            hs_line1 = menu_font.render(f"Neuer Highscore:  {new_hs}", True, hs_col)
            hs_line2 = small_font.render(f"Alter Highscore:  {old_highscore}", True, (170, 170, 170))
            base_surface.blit(hs_line1, (cx - hs_line1.get_width() // 2, HEIGHT // 2 - 52))
            base_surface.blit(hs_line2, (cx - hs_line2.get_width() // 2, HEIGHT // 2 - 20))
        else:
            hs_line = menu_font.render(f"Highscore:  {new_hs}", True, (180, 180, 210))
            base_surface.blit(hs_line, (cx - hs_line.get_width() // 2, HEIGHT // 2 - 44))

        # Coins
        gained_s = small_font.render(f"+{coins_gained} Coins", True, GOLD)
        base_surface.blit(gained_s, (cx - gained_s.get_width() // 2, HEIGHT // 2 + 20))

        # Buttons: Neustart | Menü
        btn_y      = HEIGHT // 2 + 60
        retry_rect = pygame.Rect(cx - 215, btn_y, 195, 46)
        menu_rect  = pygame.Rect(cx + 20,  btn_y, 195, 46)
        _go_rects  = [retry_rect, menu_rect]
        mp = get_mouse_pos_logical()
        retry_hl = retry_rect.collidepoint(mp) or nav_is_focused("game_over_1p", 0, len(_go_rects))
        menu_hl  = menu_rect.collidepoint(mp)  or nav_is_focused("game_over_1p", 1, len(_go_rects))
        draw_ui_button(base_surface, retry_rect, get_text("retry"), font=menu_font, hovered=retry_hl,
                        base_color=(0, 130, 60), hover_color=(30, 175, 90), text_color=(255, 255, 255),
                        border_color=(60, 90, 70), hover_border_color=(190, 255, 210), hover_border_width=3)
        draw_ui_button(base_surface, menu_rect, get_text("back"), font=menu_font, hovered=menu_hl)

        # Controls hint
        retry_s = small_font.render(f"R: {get_text('retry')}   ·   M: {get_text('back')}", True, (130, 130, 150))
        base_surface.blit(retry_s, (cx - retry_s.get_width() // 2, btn_y + 58))

        pygame.display.flip()

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_r:
                    game_running = True
                    in_menu = False
                    return
                if event.key == pygame.K_m or is_back_key(event.key):
                    in_menu = True
                    game_running = False
                    return
                d = nav_direction(event.key)
                if d:
                    nav_set_focus("game_over_1p", nav_move(_go_rects, nav_get_focus("game_over_1p", len(_go_rects)), d))
                elif is_nav_confirm(event.key):
                    i = nav_get_focus("game_over_1p", len(_go_rects))
                    if i == 0:
                        game_running = True
                        in_menu = False
                        return
                    elif i == 1:
                        in_menu = True
                        game_running = False
                        return
            if event.type == pygame.MOUSEBUTTONDOWN:
                mp2 = get_mouse_pos_logical()
                if retry_rect.collidepoint(mp2):
                    play_sound(menu_click_sound)
                    game_running = True
                    in_menu = False
                    return
                if menu_rect.collidepoint(mp2):
                    play_sound(click_back_sound)
                    in_menu = True
                    game_running = False
                    return


# =============================
# Dev Console (nur im Source-Run - die kompilierte .exe hat kein Konsolenfenster,
# siehe --windows-console-mode=disable im Windows-Build)
# =============================
# Erlaubt beim Testen, in der Konsole (NICHT im Spielfenster!) Befehle wie
# "run shop" oder "run collect_1up 3" einzutippen, um direkt zu Screens zu
# springen oder Spielzustand zu ändern, ohne sich manuell durchzuklicken.
# Jede Aktion loggt in die Konsole, was sie getan hat.

_dev_command_queue = queue.Queue()
_dev_round_actions = []  # (action, parsed_arg) - wird von run_game() pro Frame geleert und angewendet

# Die Konsole schaltet sich automatisch frei, wenn key.txt (Projekt-Root, per
# .gitignore nie committet) lokal vorhanden und lesbar ist - der Key selbst muss
# dafür NICHT eingetippt werden (er ist ~100 Zeichen lang, unpraktisch zum Abtippen/
# Einfügen ohne Zwischenablage). Wer die Datei nicht hat, hat auch keinen Zugriff -
# kein Fallback-Key im Code.
try:
    with open(os.path.join(BASE_DIR, "key.txt"), "r", encoding="utf-8") as _f:
        _dev_console_key = _f.read().strip() or None
except Exception:
    _dev_console_key = None
_dev_console_unlocked = _dev_console_key is not None


def _dev_console_reader():
    """Läuft in einem Daemon-Thread, liest blockierend von stdin und legt jede
    Zeile in die Queue. Die eigentliche Verarbeitung passiert im Haupt-Thread
    (siehe _process_dev_commands, aufgerufen von sync_window_size), damit
    Spielzustand nie aus einem anderen Thread heraus verändert wird."""
    while True:
        try:
            line = input()
        except (EOFError, OSError, ValueError):
            return  # Keine Konsole angehängt (z.B. kompilierte exe) -> Thread beendet sich still
        except Exception:
            return
        if line.strip():
            _dev_command_queue.put(line.strip())


_ALL_SCREEN_FLAGS = [
    "in_menu", "game_running", "showing_stats", "in_shop", "in_loadout", "in_profile_select",
    "in_profile_name_input", "in_mods_menu", "in_more_menu", "in_classic_game", "in_controls_menu",
    "in_settings_menu", "in_settings_more_menu", "in_experimental_menu", "in_settings_sound_menu",
    "in_help", "in_daily_rewards", "in_shop_menu", "in_cosmetic_shop", "in_loadout_menu",
    "in_cosmetic_inventory", "in_loading", "in_difficulty_select", "in_tutorial", "in_language_select",
    "in_admin_login", "in_admin_panel", "in_achievements_screen", "in_profile_hub",
    "in_online_leaderboard", "in_player_search", "in_public_profile",
    "in_rename_profile", "in_redeem_code",
    "in_friends_hub", "in_clan_hub", "in_clan_create", "in_clan_search", "in_clan_view", "in_chat",
]


def _dev_goto(flag_name, extra=None):
    """Setzt alle Screen-Flags zurück und aktiviert nur das/die Ziel-Flag(s) - so
    landet man nie in zwei gleichzeitig 'aktiven' Screens, egal von wo aus der
    Befehl kam (jeder Menü-Screen hat seine eigene while-Schleife, siehe fill_bg)."""
    g = globals()
    for name in _ALL_SCREEN_FLAGS:
        g[name] = False
    g[flag_name] = True
    if extra:
        for k, v in extra.items():
            g[k] = v
    print(f"[DEV] -> {flag_name}" + (f" {extra}" if extra else ""))


def _dev_cmd_nav(target):
    def handler(args):
        _dev_goto(target)
    return handler

def _dev_cmd_game(args):
    _dev_goto("game_running", {"current_game_mode": "standard"})

def _dev_cmd_game2p(args):
    _dev_goto("game_running", {"current_game_mode": "2player"})


def _dev_parse_int(args, index=0, default=None):
    try:
        return int(args[index])
    except (IndexError, ValueError):
        return default


def _dev_cmd_add_coins(args):
    n = _dev_parse_int(args, default=100)
    stats["coins"] = stats.get("coins", 0) + n
    save_section("stats", stats)
    print(f"[DEV] add_coins({n}): coins={stats['coins']}")

def _dev_cmd_set_coins(args):
    n = _dev_parse_int(args, default=0)
    stats["coins"] = n
    save_section("stats", stats)
    print(f"[DEV] set_coins({n}): coins={stats['coins']}")

def _dev_cmd_unlock_all_powers(args):
    owned = powers_state.setdefault("owned", [])
    added = [p for p in POWERS_LIST if p not in owned]
    owned.extend(added)
    save_section("powers", powers_state)
    print(f"[DEV] unlock_all_powers: {len(added)} neu freigeschaltet ({len(owned)} gesamt)")

def _dev_cmd_unlock_all_cosmetics(args):
    owned = cosmetics_state.setdefault("owned", [])
    added = [k for k in COSMETICS_DB if k not in owned]
    owned.extend(added)
    save_section("cosmetics", cosmetics_state)
    print(f"[DEV] unlock_all_cosmetics: {len(added)} neu freigeschaltet ({len(owned)} gesamt)")

def _dev_cmd_give_power(args):
    if not args:
        print(f"[DEV] give_power braucht einen Namen, z.B.: run give_power Double Jump. Verfügbar: {POWERS_LIST}")
        return
    name = " ".join(args)
    matches = [p for p in POWERS_LIST if p.lower() == name.lower()]
    if not matches:
        print(f"[DEV] Unbekannte Power: {name!r}. Verfügbar: {POWERS_LIST}")
        return
    owned = powers_state.setdefault("owned", [])
    if matches[0] in owned:
        print(f"[DEV] give_power({matches[0]}): bereits besessen")
        return
    owned.append(matches[0])
    save_section("powers", powers_state)
    print(f"[DEV] give_power({matches[0]}): hinzugefügt")

def _dev_cmd_set_difficulty(args):
    global difficulty_index
    if not args:
        print(f"[DEV] set_difficulty braucht einen Namen: {difficulties}")
        return
    name = args[0]
    matches = [d for d in difficulties if d.lower() == name.lower()]
    if not matches:
        print(f"[DEV] Unbekannte Schwierigkeit: {name!r}. Verfügbar: {difficulties}")
        return
    difficulty_index = difficulties.index(matches[0])
    settings["difficulty"] = difficulties[difficulty_index]
    save_section("settings", settings)
    update_button_texts()
    print(f"[DEV] set_difficulty({matches[0]})")

def _dev_cmd_set_lang(args):
    global language_index
    if not args:
        print(f"[DEV] set_lang braucht einen Namen: {languages}")
        return
    name = args[0]
    matches = [l for l in languages if l.lower() == name.lower()]
    if not matches:
        print(f"[DEV] Unbekannte Sprache: {name!r}. Verfügbar: {languages}")
        return
    language_index = languages.index(matches[0])
    settings["language"] = languages[language_index]
    save_section("settings", settings)
    update_button_texts()
    print(f"[DEV] set_lang({matches[0]})")

def _dev_cmd_set_player_color(args):
    if not args or args[0].lower() not in PLAYER_COLORS:
        print(f"[DEV] set_player_color braucht eine Farbe: {list(PLAYER_COLORS.keys())}")
        return
    settings["player_color"] = args[0].lower()
    save_section("settings", settings)
    print(f"[DEV] set_player_color({args[0].lower()})")

def _dev_cmd_chat(args):
    """Braucht Argumente (welcher Chat), kann deshalb nicht den einfachen
    _dev_cmd_nav()-Wrapper nutzen - setzt den Chat-State direkt über
    _dev_goto()s extra-Parameter, gleiches Prinzip wie _dev_cmd_game()."""
    if len(args) < 2 or args[0] not in ("dm", "clan"):
        print("[DEV] chat braucht: run chat <dm|clan> <Name-oder-Tag>")
        return
    _dev_goto("in_chat", {
        "_chat_conversation_type": args[0],
        "_chat_target": args[1],
        "_chat_return_screen": "in_profile_hub",
        "_chat_messages": [],
        "_chat_last_id": 0,
        "_chat_last_poll": 0,
        "_chat_input_text": "",
        "_chat_msg": "",
        "_chat_loading": True,
    })


def _dev_cmd_round_action(action_name, arg_parser=None):
    """Baut einen Handler, der eine In-Runde-Aktion nur einreiht (angewendet wird
    sie erst von run_game() selbst, da score/lives/etc. dessen lokale Variablen
    sind und von hier aus nicht direkt erreichbar sind)."""
    def handler(args):
        if not game_running:
            print(f"[DEV] '{action_name}' braucht eine laufende Runde (aktuell nicht im Spiel)")
            return
        if current_game_mode == "2player":
            print(f"[DEV] '{action_name}' wird aktuell nur im Singleplayer unterstützt")
            return
        parsed = arg_parser(args) if arg_parser else None
        _dev_round_actions.append((action_name, parsed))
        print(f"[DEV] {action_name} eingereiht, wird im nächsten Frame angewendet...")
    return handler


DEV_COMMANDS = {
    # Navigation - springt direkt zu einem Screen
    "menu":               (_dev_cmd_nav("in_menu"), "Zurück zum Hauptmenü"),
    "shop":               (_dev_cmd_nav("in_shop_menu"), "Shop-Hub öffnen"),
    "shop_roll":          (_dev_cmd_nav("in_shop"), "Power-Roll-Shop öffnen"),
    "shop_cosmetic":      (_dev_cmd_nav("in_cosmetic_shop"), "Kosmetik-Shop öffnen"),
    "loadout":            (_dev_cmd_nav("in_loadout_menu"), "Loadout-Hub öffnen"),
    "loadout_powers":     (_dev_cmd_nav("in_loadout"), "Power-Loadout öffnen"),
    "cosmetic_inventory": (_dev_cmd_nav("in_cosmetic_inventory"), "Kosmetik-Inventar öffnen"),
    "settings":           (_dev_cmd_nav("in_settings_menu"), "Einstellungen öffnen"),
    "controls":           (_dev_cmd_nav("in_controls_menu"), "Steuerung-Screen öffnen"),
    "more":               (_dev_cmd_nav("in_more_menu"), "Mehr-Menü öffnen"),
    "profile_hub":        (_dev_cmd_nav("in_profile_hub"), "Profil-Hub öffnen (Avatar/Stats/Profile)"),
    "online_leaderboard": (_dev_cmd_nav("in_online_leaderboard"), "Online-Rangliste öffnen"),
    "player_search":      (_dev_cmd_nav("in_player_search"), "Spielersuche öffnen"),
    "friends_hub":        (_dev_cmd_nav("in_friends_hub"), "Freunde-Hub öffnen (Freunde/Anfragen)"),
    "clan_hub":           (_dev_cmd_nav("in_clan_hub"), "Clan-Hub öffnen"),
    "chat":               (_dev_cmd_chat, "Chat öffnen, z.B.: run chat dm SomeFriend / run chat clan TAG"),
    "rename_profile":     (_dev_cmd_nav("in_rename_profile"), "Namen-ändern-Screen öffnen"),
    "redeem_code":        (_dev_cmd_nav("in_redeem_code"), "Code-Einlösen-Screen öffnen (im Mehr-Menü)"),
    "stats":              (_dev_cmd_nav("showing_stats"), "Statistik-Screen öffnen"),
    "difficulty":         (_dev_cmd_nav("in_difficulty_select"), "Schwierigkeitsauswahl öffnen"),
    "daily_rewards":      (_dev_cmd_nav("in_daily_rewards"), "Daily Rewards öffnen"),
    "admin_login":        (_dev_cmd_nav("in_admin_login"), "Admin-Login öffnen"),
    "admin_panel":        (_dev_cmd_nav("in_admin_panel"), "Admin-Panel öffnen (Login übersprungen)"),
    "mods":               (_dev_cmd_nav("in_mods_menu"), "Mods-Menü öffnen"),
    "help":               (_dev_cmd_nav("in_help"), "Hilfe-Screen öffnen"),
    "tutorial":           (_dev_cmd_nav("in_tutorial"), "Tutorial öffnen"),
    "achievements":       (_dev_cmd_nav("in_achievements_screen"), "Achievements-Screen öffnen (im Spiel normal nicht erreichbar)"),
    "language_select":    (_dev_cmd_nav("in_language_select"), "Sprachauswahl öffnen"),
    "game":               (_dev_cmd_game, "Direkt eine Standard-Runde starten"),
    "game2p":             (_dev_cmd_game2p, "Direkt eine 2-Spieler-Runde starten"),

    # Globaler Zustand (Coins, Powers, Cosmetics, Schwierigkeit, Sprache)
    "add_coins":            (_dev_cmd_add_coins, "Coins hinzufügen, z.B.: run add_coins 500"),
    "set_coins":            (_dev_cmd_set_coins, "Coins setzen, z.B.: run set_coins 0"),
    "unlock_all_powers":    (_dev_cmd_unlock_all_powers, "Alle Powers freischalten"),
    "unlock_all_cosmetics": (_dev_cmd_unlock_all_cosmetics, "Alle Kosmetik-Items freischalten"),
    "give_power":           (_dev_cmd_give_power, "Eine bestimmte Power geben, z.B.: run give_power Double Jump"),
    "set_difficulty":       (_dev_cmd_set_difficulty, "Schwierigkeit setzen, z.B.: run set_difficulty Hardcore"),
    "set_lang":             (_dev_cmd_set_lang, "Sprache setzen, z.B.: run set_lang English"),
    "set_player_color":     (_dev_cmd_set_player_color, "Avatar-/Spielerfarbe setzen, z.B.: run set_player_color red"),

    # In-Runde (braucht laufendes Spiel, aktuell nur Singleplayer-Standardmodus)
    "collect_1up": (_dev_cmd_round_action("collect_1up", lambda a: _dev_parse_int(a, default=1)),
                     "1-UP(s) einsammeln, z.B.: run collect_1up 3"),
    "add_score":   (_dev_cmd_round_action("add_score", lambda a: _dev_parse_int(a, default=10)),
                     "Score erhöhen, z.B.: run add_score 100"),
    "add_lives":   (_dev_cmd_round_action("add_lives", lambda a: _dev_parse_int(a, default=1)),
                     "Leben hinzufügen, z.B.: run add_lives 1"),
    "set_lives":   (_dev_cmd_round_action("set_lives", lambda a: _dev_parse_int(a, default=0)),
                     "Leben setzen, z.B.: run set_lives 0 (Runde endet beim nächsten Treffer)"),
    "god_mode":    (_dev_cmd_round_action("god_mode", lambda a: (a[0].lower() != "off") if a else True),
                     "Unverwundbarkeit an/aus, z.B.: run god_mode / run god_mode off"),
}


def _dev_cmd_help(args):
    print("[DEV] Verfügbare Befehle (Aufruf: run <befehl> [args]):")
    for name in sorted(DEV_COMMANDS):
        _, desc = DEV_COMMANDS[name]
        print(f"  {name:<22} {desc}")

def _dev_cmd_state(args):
    active = [f for f in _ALL_SCREEN_FLAGS if globals().get(f)]
    print(f"[DEV] Aktive Screen-Flags: {active}")
    print(f"[DEV] mode={current_game_mode!r}  language={languages[language_index]!r}  "
          f"difficulty={difficulties[difficulty_index]!r}  coins={stats.get('coins', 0)}")

DEV_COMMANDS["help"]  = (_dev_cmd_help, "Diese Liste anzeigen")
DEV_COMMANDS["list"]  = (_dev_cmd_help, "Alias für help")
DEV_COMMANDS["state"] = (_dev_cmd_state, "Aktuellen Zustand anzeigen")


def _dispatch_dev_command(line):
    if not _dev_console_unlocked:
        print("[DEV] Konsole deaktiviert (key.txt nicht gefunden).")
        return

    parts = line.split()
    if not parts:
        return
    if parts[0] != "run":
        print(f"[DEV] Unbekannte Eingabe (Befehle starten mit 'run', z.B. 'run help'): {line!r}")
        return
    if len(parts) < 2:
        print("[DEV] 'run' braucht einen Befehlsnamen, z.B.: run help")
        return
    cmd, args = parts[1], parts[2:]
    entry = DEV_COMMANDS.get(cmd)
    if entry is None:
        print(f"[DEV] Unbekannter Befehl: {cmd!r}. 'run help' für die Liste.")
        return
    handler, _desc = entry
    try:
        handler(args)
    except Exception as e:
        print(f"[DEV] Fehler beim Ausführen von '{cmd}': {e}")


def _process_dev_commands():
    """Von sync_window_size() jeden Frame aufgerufen - leert die Queue vom Reader-Thread."""
    while True:
        try:
            line = _dev_command_queue.get_nowait()
        except queue.Empty:
            return
        _dispatch_dev_command(line)


def _drain_dev_round_actions():
    actions = _dev_round_actions[:]
    _dev_round_actions.clear()
    return actions


threading.Thread(target=_dev_console_reader, daemon=True).start()
if _dev_console_unlocked:
    print("[DEV] Konsole aktiv (key.txt gefunden) - 'run help' fuer alle Befehle.")
else:
    print("[DEV] Dev-Konsole deaktiviert (key.txt nicht gefunden).")


# =============================
# Main Loop
# =============================


update_button_texts()
clock = pygame.time.Clock()

while True:
    sync_window_size()
    if in_profile_name_input:
        confirm_rect = draw_profile_name_input()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_RETURN:
                    _confirm_profile_name()
                elif event.key == pygame.K_BACKSPACE:
                    _profile_name_text = _profile_name_text[:-1]
                elif is_back_key(event.key, allow_backspace=False):
                    if get_all_profiles():
                        in_profile_name_input = False
                        in_profile_select = True
                else:
                    ch = event.unicode
                    if ch and 32 <= ord(ch) <= 126 and len(_profile_name_text) < 20:
                        _profile_name_text += ch
            if event.type == pygame.MOUSEBUTTONDOWN:
                if confirm_rect.collidepoint(get_mouse_pos_logical()):
                    _confirm_profile_name()
        clock.tick(60)

    elif in_profile_select:
        card_rects_ps, checkbox_rects_ps = draw_profile_select()
        _ps_rects = [r for r, _n in card_rects_ps]
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                d = nav_direction(event.key)
                if d:
                    cur = nav_get_focus("profile_select", len(_ps_rects))
                    nav_set_focus("profile_select", nav_move(_ps_rects, cur, d))
                elif is_nav_confirm(event.key):
                    _fi = nav_get_focus("profile_select", len(_ps_rects))
                    if _fi is not None and _fi < len(card_rects_ps):
                        name = card_rects_ps[_fi][1]
                        if name == "+":
                            play_sound(menu_click_sound)
                            _open_new_profile_input(from_settings=True)
                        else:
                            play_sound(menu_click_sound)
                            _enter_profile(name)
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                # "Immer mit diesem Profil starten"-Checkbox hat Vorrang vor dem
                # Karten-Klick darunter, sonst würde man beim Ankreuzen versehentlich
                # gleich das Profil betreten.
                _clicked_checkbox = False
                for cb_rect, cb_name in checkbox_rects_ps:
                    if cb_rect.collidepoint((mx, my)):
                        current_auto = get_auto_start_profile()
                        set_auto_start_profile(None if current_auto == cb_name else cb_name)
                        play_sound(click_sound)
                        _clicked_checkbox = True
                        break
                if _clicked_checkbox:
                    continue
                for rect, name in card_rects_ps:
                    if rect.collidepoint((mx, my)):
                        if name == "+":
                            play_sound(menu_click_sound)
                            _open_new_profile_input(from_settings=True)
                        else:
                            # Check delete button
                            del_rect = pygame.Rect(rect.right - 28, rect.y + 6, 22, 22)
                            if del_rect.collidepoint((mx, my)):
                                play_sound(click_back_sound)
                                delete_profile(name)
                                if not get_all_profiles():
                                    in_profile_select = False
                                    in_profile_name_input = True
                            else:
                                play_sound(menu_click_sound)
                                _enter_profile(name)
                        break
        clock.tick(60)

    elif in_language_select:
        weiter_rect, light_rect, dark_rect, card_rects = draw_language_select()
        _ls_rects = list(card_rects)
        if not _lang_from_settings:
            _ls_rects += [light_rect, dark_rect]
        _ls_rects += [weiter_rect]
        _ls_weiter_idx = len(_ls_rects) - 1
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                d = nav_direction(event.key)
                if d:
                    cur = nav_get_focus("language_select", len(_ls_rects), default_idx=_ls_weiter_idx)
                    nav_set_focus("language_select", nav_move(_ls_rects, cur, d))
                elif is_nav_confirm(event.key):
                    _fi = nav_get_focus("language_select", len(_ls_rects), default_idx=_ls_weiter_idx)
                    if _fi == _ls_weiter_idx:
                        _finish_language_select()
                    elif _fi is not None and _fi < len(card_rects):
                        _select_language(_LANG_DATA[_fi]["key"])
                        play_sound(click_sound)
                    elif not _lang_from_settings and _fi == len(card_rects):
                        settings["dark_mode"] = False
                        save_section("settings", settings)
                        play_sound(click_sound)
                    elif not _lang_from_settings and _fi == len(card_rects) + 1:
                        settings["dark_mode"] = True
                        save_section("settings", settings)
                        play_sound(click_sound)
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                pt = (mx, my)
                # Sprach-Karte klicken
                for i, (ld, cr) in enumerate(zip(_LANG_DATA, card_rects)):
                    if cr.collidepoint(pt):
                        _select_language(ld["key"])
                        play_sound(click_sound)
                        break
                # Hell/Dunkel-Toggle
                if light_rect.collidepoint(pt):
                    settings["dark_mode"] = False
                    save_section("settings", settings)
                    play_sound(click_sound)
                elif dark_rect.collidepoint(pt):
                    settings["dark_mode"] = True
                    save_section("settings", settings)
                    play_sound(click_sound)
                # Weiter-Button
                if weiter_rect.collidepoint(pt):
                    play_sound(menu_click_sound)
                    _finish_language_select()
        clock.tick(60)

    elif in_tutorial:
        steps = _get_tutorial_steps()
        total_steps = len(steps)
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if is_back_key(event.key):
                    _finish_tutorial()
                elif event.key in (pygame.K_RIGHT, pygame.K_d, pygame.K_RETURN, pygame.K_SPACE):
                    if tutorial_step >= total_steps - 1:
                        _finish_tutorial()
                    else:
                        tutorial_step += 1
                elif event.key in (pygame.K_LEFT, pygame.K_a):
                    if tutorial_step > 0:
                        tutorial_step -= 1
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                panel_w, panel_h = 560, 320
                panel_x = WIDTH // 2 - panel_w // 2
                panel_y = HEIGHT // 2 - panel_h // 2
                btn_y = panel_y + panel_h - 52
                btn_h = 40
                next_rect = pygame.Rect(panel_x + panel_w - 160, btn_y, 140, btn_h)
                back_rect = pygame.Rect(panel_x + 20, btn_y, 140, btn_h)
                if next_rect.collidepoint((mx, my)):
                    if tutorial_step >= total_steps - 1:
                        _finish_tutorial()
                    else:
                        tutorial_step += 1
                elif back_rect.collidepoint((mx, my)) and tutorial_step > 0:
                    tutorial_step -= 1
        draw_tutorial()
        clock.tick(60)

    elif in_menu:
        _menu_has_mods = len(mod_system.mods) > 0
        daily_reward_rect = pygame.Rect(WIDTH - 200, 15, 180, 40)
        _mods_btn_check = pygame.Rect(20, HEIGHT - 56, 130, 36)
        _profile_hub_btn_rect = _get_profile_hub_btn_rect()
        _menu_rects = [b.rect for b in buttons]
        _menu_acts  = [b.callback for b in buttons]
        if _menu_has_mods:
            _menu_rects.append(_mods_btn_check)
            _menu_acts.append(open_mods_menu)
        _menu_rects.append(daily_reward_rect)
        _menu_acts.append(open_daily_rewards_menu)
        _menu_rects.append(_profile_hub_btn_rect)
        _menu_acts.append(open_profile_hub)

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                d = nav_direction(event.key)
                if d:
                    cur = nav_get_focus("menu", len(_menu_rects))
                    nav_set_focus("menu", nav_move(_menu_rects, cur, d))
                elif is_nav_confirm(event.key):
                    i = nav_get_focus("menu", len(_menu_rects))
                    if i is not None and 0 <= i < len(_menu_acts):
                        _menu_acts[i]()
            mouse_pos = get_mouse_pos_logical()
            for button in buttons:
                button.handle_event(event, mouse_pos)

            # Daily Rewards Button
            if event.type == pygame.MOUSEBUTTONDOWN:
                if daily_reward_rect.collidepoint(mouse_pos):
                    play_sound(menu_click_sound)
                    open_daily_rewards_menu()
                # Mods Button
                if _menu_has_mods:
                    if _mods_btn_check.collidepoint(mouse_pos):
                        open_mods_menu()
                # Profil-Button
                if _profile_hub_btn_rect.collidepoint(mouse_pos):
                    open_profile_hub()
        draw_menu()
        clock.tick(60)

    elif in_difficulty_select:
        # ---- Registry für Tastatur-Fokus (gleiche Geometrie wie draw_difficulty_select) ----
        SIDEBAR_X, SIDEBAR_W = 24, 180
        MAIN_X = SIDEBAR_X + SIDEBAR_W + 24
        MAIN_W = max(300, WIDTH - MAIN_X - 24)
        MAIN_Y0 = 118
        custom_btn_rect   = pygame.Rect(SIDEBAR_X, MAIN_Y0, SIDEBAR_W, 56)
        _presets_lbl_y    = custom_btn_rect.bottom + 20
        preset_btn_y      = _presets_lbl_y + 22
        standard_btn_rect = pygame.Rect(SIDEBAR_X, preset_btn_y, SIDEBAR_W, 42)
        klassik_btn_rect  = pygame.Rect(SIDEBAR_X, standard_btn_rect.bottom + 8, SIDEBAR_W, 42)
        tp_btn_rect       = pygame.Rect(SIDEBAR_X, klassik_btn_rect.bottom + 8, SIDEBAR_W, 42)
        _ds_sidebar_rects = [custom_btn_rect, standard_btn_rect, klassik_btn_rect, tp_btn_rect]
        if current_game_mode == "2player":
            _sub_w = (SIDEBAR_W - 8) // 2
            _sub_y = tp_btn_rect.bottom + 10
            tab_coop_rect = pygame.Rect(SIDEBAR_X, _sub_y, _sub_w, 32)
            tab_vs_rect   = pygame.Rect(SIDEBAR_X + _sub_w + 8, _sub_y, _sub_w, 32)
            _ds_sidebar_rects += [tab_coop_rect, tab_vs_rect]
        _ds_toggle_defs = [
            ("custom_oneups", True), ("custom_vertical_obstacles", False),
            ("custom_wind_sides", False), ("custom_wind_all", False),
            ("custom_free_roam", False),
        ]
        if current_game_mode == "custom":
            _ds_slider_rects = [pygame.Rect(MAIN_X, MAIN_Y0 + sy - 20, MAIN_W, 40) for sy in (30, 100, 170)]
            _mod_y, _tw, _th, _tgap, _row_gap, _cols = MAIN_Y0 + 210, 220, 34, 12, 10, 2
            _toggle_y = _mod_y + 34
            _ds_toggle_rects = [pygame.Rect(MAIN_X + (ti % _cols) * (_tw + _tgap), _toggle_y + (ti // _cols) * (_th + _row_gap), _tw, _th)
                               for ti in range(len(_ds_toggle_defs))]
            _ds_main_rects = _ds_slider_rects + _ds_toggle_rects
        else:
            _card_h, _gap = 50, 56
            _ds_main_rects = [pygame.Rect(MAIN_X, MAIN_Y0 + i * _gap, MAIN_W, _card_h) for i in range(len(difficulties))]
        _ds_btn_y = HEIGHT - 54
        _ds_start_rect = pygame.Rect(WIDTH // 2 + 8,  _ds_btn_y, 200, 44)
        _ds_back_rect  = pygame.Rect(WIDTH // 2 - 208, _ds_btn_y, 200, 44)
        _ds_rects = _ds_sidebar_rects + _ds_main_rects + [_ds_start_rect, _ds_back_rect]
        _ds_sidebar_count = len(_ds_sidebar_rects)
        _ds_start_idx = _ds_sidebar_count + len(_ds_main_rects)
        _ds_back_idx  = _ds_start_idx + 1

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if _ds_slider_edit is not None:
                    _sdir = nav_direction(event.key)
                    if _sdir in ("left", "right"):
                        _sdelta = -1 if _sdir == "left" else 1
                        if _ds_slider_edit == 0:
                            _scur = settings.get("custom_speed", 10)
                            settings["custom_speed"] = max(4, min(35, _scur + _sdelta * 1))
                        elif _ds_slider_edit == 1:
                            _scur = settings.get("custom_min_delay", 900)
                            _smax_allowed = settings.get("custom_max_delay", 1600) - 100
                            settings["custom_min_delay"] = max(100, min(_scur + _sdelta * 50, _smax_allowed))
                        elif _ds_slider_edit == 2:
                            _scur = settings.get("custom_max_delay", 1600)
                            _smin_allowed = settings.get("custom_min_delay", 900) + 100
                            settings["custom_max_delay"] = min(2500, max(_scur + _sdelta * 50, _smin_allowed))
                        save_section("settings", settings)
                    elif is_nav_confirm(event.key) or is_back_key(event.key):
                        _ds_slider_edit = None
                elif event.key == pygame.K_m or is_back_key(event.key):
                    in_difficulty_select = False
                    in_menu = True
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("difficulty_select", len(_ds_rects))
                        nav_set_focus("difficulty_select", nav_move(_ds_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("difficulty_select", len(_ds_rects))
                        if _fi is not None and _fi < 4:
                            current_game_mode = ["custom", "standard", "klassik", "2player"][_fi]
                            klassik_mode = (current_game_mode == "klassik")
                            settings["game_mode"] = current_game_mode
                            if current_game_mode != "custom":
                                _dpm = settings.setdefault("difficulty_per_mode", {"standard": "Mittel", "klassik": "Mittel", "2player": "Mittel"})
                                diff_for_mode = _dpm.get(current_game_mode, "Mittel")
                                if diff_for_mode in difficulties:
                                    difficulty_index = difficulties.index(diff_for_mode)
                                settings["difficulty"] = difficulties[difficulty_index]
                            save_section("settings", settings)
                            update_button_texts()
                            play_sound(click_sound)
                            nav_set_focus("difficulty_select", _fi)
                            _ds_slider_edit = None
                        elif current_game_mode == "2player" and _fi == 4:
                            two_player_submode = "coop"
                            settings["2player_submode"] = "coop"
                            save_section("settings", settings)
                            play_sound(click_sound)
                        elif current_game_mode == "2player" and _fi == 5:
                            two_player_submode = "vs"
                            settings["2player_submode"] = "vs"
                            save_section("settings", settings)
                            play_sound(click_sound)
                        elif _fi == _ds_start_idx:
                            actually_start_game()
                        elif _fi == _ds_back_idx:
                            play_sound(click_back_sound)
                            in_difficulty_select = False
                            in_menu = True
                        elif _fi is not None and _ds_sidebar_count <= _fi < _ds_start_idx:
                            _mi = _fi - _ds_sidebar_count
                            if current_game_mode == "custom" and _mi < 3:
                                _ds_slider_edit = _mi
                                play_sound(click_sound)
                            elif current_game_mode == "custom":
                                skey, default = _ds_toggle_defs[_mi - 3]
                                new_val = not settings.get(skey, default)
                                settings[skey] = new_val
                                if new_val and skey == "custom_wind_sides":
                                    settings["custom_wind_all"] = False
                                elif new_val and skey == "custom_wind_all":
                                    settings["custom_wind_sides"] = False
                                save_section("settings", settings)
                                play_sound(click_sound)
                            else:
                                difficulty_index = _mi
                                settings["difficulty"] = difficulties[_mi]
                                _dpm = settings.setdefault("difficulty_per_mode", {"standard": "Mittel", "klassik": "Mittel", "2player": "Mittel"})
                                _dpm[current_game_mode] = difficulties[_mi]
                                save_section("settings", settings)
                                update_button_texts()
                                play_sound(click_sound)
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                tab_clicked = False
                _clicked_tab_idx = None
                if custom_btn_rect.collidepoint((mx, my)):
                    current_game_mode = "custom"
                    klassik_mode = False
                    tab_clicked = True
                    _clicked_tab_idx = 0
                    play_sound(click_sound)
                elif standard_btn_rect.collidepoint((mx, my)):
                    current_game_mode = "standard"
                    klassik_mode = False
                    tab_clicked = True
                    _clicked_tab_idx = 1
                    play_sound(click_sound)
                elif klassik_btn_rect.collidepoint((mx, my)):
                    current_game_mode = "klassik"
                    klassik_mode = True
                    tab_clicked = True
                    _clicked_tab_idx = 2
                    play_sound(click_sound)
                elif tp_btn_rect.collidepoint((mx, my)):
                    current_game_mode = "2player"
                    klassik_mode = False
                    tab_clicked = True
                    _clicked_tab_idx = 3
                    play_sound(click_sound)
                if tab_clicked:
                    settings["game_mode"] = current_game_mode
                    if current_game_mode != "custom":
                        # Beim Moduswechsel die für diesen Modus gespeicherte Schwierigkeit laden
                        _dpm = settings.setdefault("difficulty_per_mode", {"standard": "Mittel", "klassik": "Mittel", "2player": "Mittel"})
                        diff_for_mode = _dpm.get(current_game_mode, "Mittel")
                        if diff_for_mode in difficulties:
                            difficulty_index = difficulties.index(diff_for_mode)
                        settings["difficulty"] = difficulties[difficulty_index]
                    save_section("settings", settings)
                    update_button_texts()
                    nav_set_focus("difficulty_select", _clicked_tab_idx)
                    _ds_slider_edit = None
                # ---- Co-op / VS Sub-Buttons (nur wenn 2 Player aktiv) ----
                if current_game_mode == "2player":
                    sub_w = (SIDEBAR_W - 8) // 2
                    sub_y = tp_btn_rect.bottom + 10
                    tab_coop_rect = pygame.Rect(SIDEBAR_X, sub_y, sub_w, 32)
                    tab_vs_rect   = pygame.Rect(SIDEBAR_X + sub_w + 8, sub_y, sub_w, 32)
                    if tab_coop_rect.collidepoint((mx, my)):
                        two_player_submode = "coop"
                        settings["2player_submode"] = "coop"
                        save_section("settings", settings)
                        play_sound(click_sound)
                    elif tab_vs_rect.collidepoint((mx, my)):
                        two_player_submode = "vs"
                        settings["2player_submode"] = "vs"
                        save_section("settings", settings)
                        play_sound(click_sound)
                if current_game_mode == "custom":
                    # ---- Custom-Modus: Slider-Griffe + Modifier-Toggles ----
                    card_w = MAIN_W
                    card_x = MAIN_X
                    cards_y0 = MAIN_Y0
                    _label_w = max(menu_font.size(l)[0] for l in ["Geschwindigkeit:", "Min-Pause:", "Max-Pause:"])
                    slider_x = card_x + _label_w + 24
                    slider_w = card_w - (_label_w + 24) - 80

                    def _handle_x(value, lo, hi):
                        frac = max(0.0, min(1.0, (value - lo) / (hi - lo)))
                        return int(slider_x + frac * slider_w)

                    y_speed, y_min, y_max = cards_y0 + 30, cards_y0 + 100, cards_y0 + 170
                    hx_speed = _handle_x(settings.get("custom_speed", 10), 4, 35)
                    hx_min   = _handle_x(settings.get("custom_min_delay", 900), 100, 2000)
                    hx_max   = _handle_x(settings.get("custom_max_delay", 1600), 200, 2500)

                    if abs(mx - hx_speed) <= 14 and abs(my - y_speed) <= 14:
                        _custom_speed_dragging = True
                    elif abs(mx - hx_min) <= 14 and abs(my - y_min) <= 14:
                        _custom_min_delay_dragging = True
                    elif abs(mx - hx_max) <= 14 and abs(my - y_max) <= 14:
                        _custom_max_delay_dragging = True
                    else:
                        mod_y = cards_y0 + 210
                        toggle_y = mod_y + 34
                        tw, th, tgap, row_gap, cols = 220, 34, 12, 10, 2
                        toggle_defs = [
                            ("custom_oneups", True), ("custom_vertical_obstacles", False),
                            ("custom_wind_sides", False), ("custom_wind_all", False),
                            ("custom_free_roam", False),
                        ]
                        for ti, (skey, default) in enumerate(toggle_defs):
                            col, row = ti % cols, ti // cols
                            tr = pygame.Rect(card_x + col * (tw + tgap), toggle_y + row * (th + row_gap), tw, th)
                            if tr.collidepoint((mx, my)):
                                new_val = not settings.get(skey, default)
                                settings[skey] = new_val
                                # Die zwei Wind-Varianten schließen sich gegenseitig aus
                                if new_val and skey == "custom_wind_sides":
                                    settings["custom_wind_all"] = False
                                elif new_val and skey == "custom_wind_all":
                                    settings["custom_wind_sides"] = False
                                save_section("settings", settings)
                                play_sound(click_sound)
                                break
                    # ---- Start / Back (Custom hat keine Karten-Liste) ----
                    btn_y = HEIGHT - 54
                    start_rect = pygame.Rect(WIDTH // 2 + 8,   btn_y, 200, 44)
                    back_rect  = pygame.Rect(WIDTH // 2 - 208, btn_y, 200, 44)
                    if start_rect.collidepoint((mx, my)):
                        actually_start_game()
                    elif back_rect.collidepoint((mx, my)):
                        play_sound(click_back_sound)
                        in_difficulty_select = False
                        in_menu = True
                else:
                    # ---- Schwierigkeitskarten ----
                    card_w = MAIN_W
                    card_x = MAIN_X
                    cards_y0 = MAIN_Y0
                    card_h = 50
                    gap = 56
                    clicked_diff = False
                    for i, diff in enumerate(difficulties):
                        card_rect = pygame.Rect(card_x, cards_y0 + i * gap, card_w, card_h)
                        if card_rect.collidepoint((mx, my)):
                            difficulty_index = i
                            settings["difficulty"] = difficulties[i]
                            # Pro Modus separat speichern
                            _dpm = settings.setdefault("difficulty_per_mode", {"standard": "Mittel", "klassik": "Mittel", "2player": "Mittel"})
                            _dpm[current_game_mode] = difficulties[i]
                            save_section("settings", settings)
                            update_button_texts()
                            play_sound(click_sound)
                            clicked_diff = True
                            break
                    if not clicked_diff:
                        btn_y = HEIGHT - 54
                        start_rect = pygame.Rect(WIDTH // 2 + 8,   btn_y, 200, 44)
                        back_rect  = pygame.Rect(WIDTH // 2 - 208, btn_y, 200, 44)
                        if start_rect.collidepoint((mx, my)):
                            actually_start_game()
                        elif back_rect.collidepoint((mx, my)):
                            play_sound(click_back_sound)
                            in_difficulty_select = False
                            in_menu = True
            if event.type == pygame.MOUSEBUTTONUP:
                _custom_speed_dragging = False
                _custom_min_delay_dragging = False
                _custom_max_delay_dragging = False
            if event.type == pygame.MOUSEMOTION and current_game_mode == "custom" and \
               (_custom_speed_dragging or _custom_min_delay_dragging or _custom_max_delay_dragging):
                mx, my = get_mouse_pos_logical()
                MAIN_X = 24 + 180 + 24
                card_w = max(300, WIDTH - MAIN_X - 24)
                card_x = MAIN_X
                _label_w = max(menu_font.size(l)[0] for l in ["Geschwindigkeit:", "Min-Pause:", "Max-Pause:"])
                slider_x = card_x + _label_w + 24
                slider_w = card_w - (_label_w + 24) - 80
                frac = max(0.0, min(1.0, (mx - slider_x) / slider_w))
                if _custom_speed_dragging:
                    settings["custom_speed"] = int(round(4 + frac * (35 - 4)))
                elif _custom_min_delay_dragging:
                    new_min = int(round(100 + frac * (2000 - 100)))
                    settings["custom_min_delay"] = max(100, min(new_min, settings.get("custom_max_delay", 1600) - 100))
                elif _custom_max_delay_dragging:
                    new_max = int(round(200 + frac * (2500 - 200)))
                    settings["custom_max_delay"] = min(2500, max(new_max, settings.get("custom_min_delay", 900) + 100))
                save_section("settings", settings)
        draw_difficulty_select()
        clock.tick(60)

    elif in_profile_hub:
        swatch_rects, swatch_names, hub_btn_rects = draw_profile_hub()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_to_main_from_profile_hub()
                else:
                    _hub_rects = swatch_rects + hub_btn_rects
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("profile_hub", len(_hub_rects))
                        nav_set_focus("profile_hub", nav_move(_hub_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _hi = nav_get_focus("profile_hub", len(_hub_rects))
                        if _hi is not None and _hi < len(swatch_names):
                            settings["player_color"] = swatch_names[_hi]
                            save_section("settings", settings)
                            play_sound(click_sound)
                        elif _hi is not None:
                            _bidx2 = _hi - len(swatch_names)
                            if _bidx2 == 0:
                                open_stats_from_profile_hub()
                            elif _bidx2 == 1:
                                open_profile_select_from_hub()
                            elif _bidx2 == 2:
                                open_online_leaderboard_from_hub()
                            elif _bidx2 == 3:
                                open_friends_hub_from_hub()
                            elif _bidx2 == 4:
                                open_clan_hub_from_hub()
                            elif _bidx2 == 5:
                                open_rename_profile_from_hub()
                            elif _bidx2 == 6:
                                back_to_main_from_profile_hub()
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                _clicked_swatch = False
                for srect, cname in zip(swatch_rects, swatch_names):
                    if srect.collidepoint((mx, my)):
                        settings["player_color"] = cname
                        save_section("settings", settings)
                        play_sound(click_sound)
                        _clicked_swatch = True
                        break
                if _clicked_swatch:
                    continue
                for idx, rect in enumerate(hub_btn_rects):
                    if rect.collidepoint((mx, my)):
                        if idx == 0:
                            open_stats_from_profile_hub()
                        elif idx == 1:
                            open_profile_select_from_hub()
                        elif idx == 2:
                            open_online_leaderboard_from_hub()
                        elif idx == 3:
                            open_friends_hub_from_hub()
                        elif idx == 4:
                            open_clan_hub_from_hub()
                        elif idx == 5:
                            open_rename_profile_from_hub()
                        elif idx == 6:
                            back_to_main_from_profile_hub()
                        break
        clock.tick(60)

    elif in_more_menu:
        start_x = WIDTH//2 - 180
        start_y = 140
        gap = 58
        if _check_admin_keyfile():
            _more_actions = ["admin", "classic", "help", "redeem_code", "back"]
        else:
            _more_actions = ["classic", "help", "redeem_code", "back"]
        _more_rects = [pygame.Rect(start_x, start_y + idx*gap, 360, 48) for idx in range(len(_more_actions))]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            _more_action = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    _more_action = "back"
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("more_menu", len(_more_rects))
                        nav_set_focus("more_menu", nav_move(_more_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _more_action = _more_actions[nav_get_focus("more_menu", len(_more_rects))]
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                for idx, rect in enumerate(_more_rects):
                    if rect.collidepoint((mx, my)):
                        _more_action = _more_actions[idx]
                        break
            if _more_action is not None:
                if _more_action == "back":
                    play_sound(click_back_sound)
                else:
                    play_sound(menu_click_sound)
                if _more_action == "admin":
                    set_more_to_admin()
                elif _more_action == "classic":
                    open_classic_game()
                elif _more_action == "help":
                    in_more_menu = False
                    in_tutorial = True
                    tutorial_step = 0
                elif _more_action == "redeem_code":
                    open_redeem_code_from_more()
                elif _more_action == "back":
                    back_to_main_from_more()
        draw_more_menu()
        clock.tick(60)

    elif in_classic_game:
        run_classic_game()

    elif in_mods_menu:
        _back_r, _hit_rects = draw_mods_menu()
        _mods_nav_rects = [r for _a, _d, r in _hit_rects] + [_back_r]
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if is_back_key(event.key, allow_backspace=(_mods_edit is None)):
                    close_mods_menu()
                elif _mods_edit is not None:
                    # Text input for string settings
                    if event.key == pygame.K_RETURN:
                        mod_idx, skey = _mods_edit
                        mod_system.mods[mod_idx].settings[skey] = _mods_edit_text
                        _mods_edit      = None
                        _mods_edit_text = ''
                        _save_mod_settings()
                    elif event.key == pygame.K_BACKSPACE:
                        _mods_edit_text = _mods_edit_text[:-1]
                    else:
                        ch = event.unicode
                        if ch and 32 <= ord(ch) <= 126:
                            _mods_edit_text += ch
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("mods_menu", len(_mods_nav_rects))
                        nav_set_focus("mods_menu", nav_move(_mods_nav_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("mods_menu", len(_mods_nav_rects))
                        if _fi is not None and _fi < len(_hit_rects):
                            _act, _dat, _r = _hit_rects[_fi]
                            _mods_handle_click(_act, _dat)
                        elif _fi == len(_hit_rects):
                            close_mods_menu()
            if event.type == pygame.MOUSEWHEEL:
                _mods_scroll = max(0, _mods_scroll - event.y * 30)
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                if _back_r.collidepoint((mx, my)):
                    close_mods_menu()
                else:
                    for action, data, rect in _hit_rects:
                        if rect.collidepoint((mx, my)):
                            _mods_handle_click(action, data)
                            break
        clock.tick(60)

    elif in_achievements_screen:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    if _ach_detail_code is not None:
                        _ach_detail_code = None
                    else:
                        in_achievements_screen = False
                        in_more_menu = True
                elif _ach_detail_code is not None:
                    if is_nav_confirm(event.key):
                        _ach_detail_code = None
                else:
                    _ach_fade_top = 75 + 18
                    _ach_fade_bottom = HEIGHT - 45
                    _ach_nav_rects = [r for r, _c in _ach_row_rects] + [pygame.Rect(WIDTH - 120, 15, 100, 35)]
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("achievements", len(_ach_nav_rects))
                        new_i = nav_move(_ach_nav_rects, cur, d)
                        if new_i != cur:
                            nav_set_focus("achievements", new_i)
                            if new_i < len(_ach_row_rects):
                                _ach_scroll = max(0, _ach_scroll + nav_reveal_delta(_ach_nav_rects[new_i], _ach_fade_top, _ach_fade_bottom))
                    elif is_nav_confirm(event.key):
                        _idx = nav_get_focus("achievements", len(_ach_nav_rects))
                        if _idx is not None and _idx < len(_ach_row_rects):
                            play_sound(menu_click_sound)
                            _ach_detail_code = _ach_row_rects[_idx][1]
                        elif _idx == len(_ach_row_rects):
                            play_sound(click_back_sound)
                            in_achievements_screen = False
                            in_more_menu = True
            if event.type == pygame.MOUSEWHEEL:
                if _ach_detail_code is None:
                    _ach_scroll = max(0, _ach_scroll - event.y * 30)
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                if _ach_detail_code is not None:
                    _ach_detail_code = None
                else:
                    back_rect = pygame.Rect(WIDTH - 120, 15, 100, 35)
                    if back_rect.collidepoint((mx, my)):
                        play_sound(click_back_sound)
                        in_achievements_screen = False
                        in_more_menu = True
                    else:
                        for row_rect, code in _ach_row_rects:
                            if row_rect.collidepoint((mx, my)):
                                play_sound(menu_click_sound)
                                _ach_detail_code = code
                                break
        draw_achievements_screen()
        clock.tick(60)

    elif in_settings_menu:
        _bg_toggle_defs2 = [
            ("bg_stars", True), ("bg_rings_enabled", True),
            ("bg_animated", True), ("mono_bg", False),
            ("show_floor_line", True),
        ]
        _toggle_total_w = 5 * 80 + 4 * 8
        _toggle_x0 = WIDTH // 2 - _toggle_total_w // 2
        _toggle_y = 155
        _sm2_toggle_rects = [pygame.Rect(_toggle_x0 + ti * 88, _toggle_y, 80, 26) for ti in range(len(_bg_toggle_defs2))]
        _sm2_start_x = WIDTH//2 - 180
        _sm2_start_y = 193
        _sm2_gap = 60
        _sm2_btn_rects = [pygame.Rect(_sm2_start_x, _sm2_start_y + idx*_sm2_gap, 360, 48) for idx in range(6)]
        _sm2_rects = _sm2_toggle_rects + _sm2_btn_rects
        _n_toggles = len(_sm2_toggle_rects)

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            _sm2_idx = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_to_main_from_settings()
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("settings_menu", len(_sm2_rects))
                        nav_set_focus("settings_menu", nav_move(_sm2_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _sm2_idx = nav_get_focus("settings_menu", len(_sm2_rects))
            mouse_pos = get_mouse_pos_logical()
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                # Check slider handle
                slider_cx = WIDTH // 2
                slider_w = 220
                track_x = slider_cx - slider_w // 2
                gui_scale = max(0.5, min(2.0, settings.get("gui_scale", 1.0)))
                t_frac = (gui_scale - 0.5) / 1.5
                hx = int(track_x + t_frac * slider_w)
                if abs(mx - hx) <= 14 and abs(my - 125) <= 14:
                    _gui_scale_dragging = True
                else:
                    for idx, rect in enumerate(_sm2_rects):
                        if rect.collidepoint((mx, my)):
                            _sm2_idx = idx
                            break
            if _sm2_idx is not None:
                if _sm2_idx < _n_toggles:
                    skey, default = _bg_toggle_defs2[_sm2_idx]
                    settings[skey] = not settings.get(skey, default)
                    save_section("settings", settings)
                    play_sound(click_sound)
                else:
                    _bidx = _sm2_idx - _n_toggles
                    if _bidx == 5:
                        play_sound(click_back_sound)
                    elif _bidx in [3, 4]:
                        play_sound(menu_click_sound)
                    else:
                        play_sound(click_sound)
                    if _bidx == 0:
                        open_language_select_from_settings()
                    elif _bidx == 1:
                        toggle_dark_mode()
                    elif _bidx == 2:
                        open_settings_sound_menu()
                    elif _bidx == 3:
                        in_settings_menu = False
                        in_controls_menu = True
                    elif _bidx == 4:
                        in_settings_menu = False
                        in_settings_more_menu = True
                    elif _bidx == 5:
                        back_to_main_from_settings()
            if event.type == pygame.MOUSEBUTTONUP:
                _gui_scale_dragging = False
            if event.type == pygame.MOUSEMOTION and _gui_scale_dragging:
                mx, my = get_mouse_pos_logical()
                slider_cx = WIDTH // 2
                slider_w = 220
                track_x = slider_cx - slider_w // 2
                frac = max(0.0, min(1.0, (mx - track_x) / slider_w))
                new_scale = round(0.5 + frac * 1.5, 2)
                settings["gui_scale"] = new_scale
                save_section("settings", settings)
        draw_settings_menu()
        clock.tick(60)

    elif in_controls_menu:
        _ctrl_key_rects, _ctrl_add_rects, _ctrl_reset_rect, _ctrl_back_rect, _ctrl_tab_rects = draw_controls_menu()
        _ctrl_tab_actions = [_STANDARD_ACTIONS, _2P_ACTIONS, _MENU_NAV_ACTIONS]
        _ctrl_nav_rects = [r for _a, r in _ctrl_add_rects] + [_ctrl_reset_rect, _ctrl_back_rect]

        def _ctrl_bind_key(action, key):
            """Bindet 'key' an 'action' - Menü-Navigations-Aktionen landen in den
            (frei entfernbaren) Extra-Tasten, alle anderen in den normalen Keybindings."""
            if action in _MENU_NAV_ACTIONS:
                extras = get_menu_nav_extra_keys()
                if key not in extras[action]:
                    extras[action].append(key)
                save_menu_nav_extra_keys(extras)
            else:
                kb = get_keybindings()
                if key not in kb[action]:
                    kb[action].append(key)
                save_keybindings(kb)

        def _ctrl_unbind_key(action, key):
            """Entfernt 'key' von 'action', falls entfernbar (feste Menü-Navigations-
            Standardtasten sind nie in _ctrl_key_rects, kommen hier also nie an)."""
            if action in _MENU_NAV_ACTIONS:
                extras = get_menu_nav_extra_keys()
                if key in extras[action]:
                    extras[action].remove(key)
                    save_menu_nav_extra_keys(extras)
            else:
                kb = get_keybindings()
                if key in kb[action]:
                    kb[action].remove(key)
                    save_keybindings(kb)

        def _ctrl_reset_active_tab():
            """Reset betrifft nur den gerade sichtbaren Tab, damit ein Klick auf 'Standard
            wiederherstellen' im 2P-Tab nicht versehentlich eigene Standard-Bindings killt."""
            if _ctrl_active_tab == 2:
                save_menu_nav_extra_keys({a: [] for a in _MENU_NAV_ACTIONS})
            else:
                kb = get_keybindings()
                for a in _ctrl_tab_actions[_ctrl_active_tab]:
                    kb[a] = list(_DEFAULT_KB[a])
                save_keybindings(kb)

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if _ctrl_listening is not None:
                    # Modal: JEDE andere Taste (auch Pfeile/Space/Enter/Backspace) wird gebunden.
                    if event.key in (pygame.K_ESCAPE, pygame.K_CAPSLOCK):
                        _ctrl_listening = None
                    else:
                        # Mehrere Actions dürfen jetzt dieselbe Taste haben —
                        # keine automatische Entfernung mehr. Konflikte werden
                        # stattdessen in draw_controls_menu() farblich markiert.
                        _ctrl_bind_key(_ctrl_listening, event.key)
                        _ctrl_listening = None
                elif event.key == pygame.K_m or is_back_key(event.key):
                    in_controls_menu = False
                    in_settings_menu = True
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("controls_menu", len(_ctrl_nav_rects))
                        nav_set_focus("controls_menu", nav_move(_ctrl_nav_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _idx = nav_get_focus("controls_menu", len(_ctrl_nav_rects))
                        if _idx is not None and _idx < len(_ctrl_add_rects):
                            _ctrl_listening = _ctrl_add_rects[_idx][0]
                            play_sound(menu_click_sound)
                        elif _idx == len(_ctrl_add_rects):
                            _ctrl_reset_active_tab()
                            play_sound(click_sound)
                        elif _idx == len(_ctrl_add_rects) + 1:
                            in_controls_menu = False
                            in_settings_menu = True
                            play_sound(click_back_sound)
            if event.type == pygame.MOUSEWHEEL:
                _ctrl_scroll -= event.y * 40
            if event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = get_mouse_pos_logical()
                clicked = False
                for _ti, _trect in enumerate(_ctrl_tab_rects):
                    if _trect.collidepoint((mx, my)):
                        if _ti != _ctrl_active_tab:
                            _ctrl_active_tab = _ti
                            _ctrl_scroll = 0
                            play_sound(click_sound)
                        clicked = True
                        break
                if not clicked:
                    for action, kcode, badge_rect in _ctrl_key_rects:
                        if badge_rect.collidepoint((mx, my)):
                            _ctrl_unbind_key(action, kcode)
                            play_sound(click_sound)
                            clicked = True
                            break
                if not clicked:
                    for action, plus_rect in _ctrl_add_rects:
                        if plus_rect.collidepoint((mx, my)):
                            _ctrl_listening = action
                            play_sound(menu_click_sound)
                            clicked = True
                            break
                if not clicked:
                    if _ctrl_reset_rect.collidepoint((mx, my)):
                        _ctrl_reset_active_tab()
                        play_sound(click_sound)
                    elif _ctrl_back_rect.collidepoint((mx, my)):
                        in_controls_menu = False
                        in_settings_menu = True
                        play_sound(click_back_sound)
        clock.tick(60)

    elif in_settings_more_menu:
        _more_rects = draw_settings_more_menu()
        _sme_actions = ["experimental", "export_lang", "back"]
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            _sme_action = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    _sme_action = "back"
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("settings_more_menu", len(_more_rects))
                        nav_set_focus("settings_more_menu", nav_move(_more_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _idx = nav_get_focus("settings_more_menu", len(_more_rects))
                        if _idx is not None and _idx < len(_sme_actions):
                            _sme_action = _sme_actions[_idx]
            elif event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
                mx, my = get_mouse_pos_logical()
                if len(_more_rects) > 0 and _more_rects[0].collidepoint((mx, my)):
                    _sme_action = "experimental"
                elif len(_more_rects) > 1 and _more_rects[1].collidepoint((mx, my)):
                    _sme_action = "export_lang"
                elif len(_more_rects) > 2 and _more_rects[2].collidepoint((mx, my)):
                    _sme_action = "back"
            if _sme_action == "experimental":
                play_sound(menu_click_sound)
                in_settings_more_menu = False
                in_experimental_menu = True
            elif _sme_action == "export_lang":
                play_sound(click_sound)
                export_current_language()
            elif _sme_action == "back":
                play_sound(click_back_sound)
                in_settings_more_menu = False
                in_settings_menu = True
        clock.tick(60)

    elif in_experimental_menu:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key) or is_nav_confirm(event.key):
                    back_to_settings_from_experimental()
            if event.type == pygame.MOUSEBUTTONDOWN:
                mouse_pos = get_mouse_pos_logical()
                mx, my = mouse_pos
                start_x = WIDTH//2 - 180
                start_y = 180
                gap = 64
                rect1 = pygame.Rect(start_x, start_y, 360, 48)
                if rect1.collidepoint((mx, my)):
                    play_sound(click_back_sound)
                    back_to_settings_from_experimental()
        draw_experimental_menu()
        clock.tick(60)

    elif in_settings_sound_menu:
        toggle_rect, back_rect, minus_rects, plus_rects, track_rects = draw_settings_sound_menu()
        _ssm_items  = [(toggle_rect, "toggle", None), (back_rect, "back", None)]
        _ssm_items += [(r, "minus", (key, delta)) for r, key, delta in minus_rects]
        _ssm_items += [(r, "plus", (key, delta)) for r, key, delta in plus_rects]
        _ssm_items += [(r, "track", track_key) for r, track_key in track_rects]
        _ssm_rects = [it[0] for it in _ssm_items]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            _ssm_idx = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    back_to_settings_from_sound()
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("settings_sound_menu", len(_ssm_rects))
                        nav_set_focus("settings_sound_menu", nav_move(_ssm_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _ssm_idx = nav_get_focus("settings_sound_menu", len(_ssm_rects))
            if event.type == pygame.MOUSEBUTTONDOWN:
                mouse_pos = get_mouse_pos_logical()
                for idx, (rect, _kind, _payload) in enumerate(_ssm_items):
                    if rect.collidepoint(mouse_pos):
                        _ssm_idx = idx
                        break
            if _ssm_idx is not None:
                kind, payload = _ssm_items[_ssm_idx][1], _ssm_items[_ssm_idx][2]
                if kind == "toggle":
                    play_sound(click_sound, "click_volume")
                    toggle_sound_enabled()
                elif kind == "back":
                    play_sound(click_back_sound, "click_volume")
                    back_to_settings_from_sound()
                elif kind in ("minus", "plus"):
                    key, delta = payload
                    play_sound(click_sound, "click_volume")
                    if key == "menu_click_volume":
                        adjust_menu_click_volume(delta)
                    elif key == "click_volume":
                        adjust_click_volume(delta)
                    elif key == "click_back_volume":
                        adjust_click_back_volume(delta)
                    elif key == "hit_volume":
                        adjust_hit_volume(delta)
                    elif key == "gameover_volume":
                        adjust_gameover_volume(delta)
                    elif key == "menu_music_volume":
                        adjust_menu_music_volume(delta)
                    elif key == "ingame_music_volume":
                        adjust_ingame_music_volume(delta)
                elif kind == "track":
                    track_key = payload
                    settings["ingame_music_track"] = track_key
                    save_section("settings", settings)
                    preview_ingame_music_track(track_key)
        clock.tick(60)

    elif in_help:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    in_help = False
                    in_menu = True
                elif event.key == pygame.K_UP:
                    help_scroll = max(0, help_scroll - 28)
                elif event.key == pygame.K_DOWN:
                    help_scroll = help_scroll + 28
            if event.type == pygame.MOUSEBUTTONDOWN:
                # scroll by wheel not supported here; ignore
                pass
        draw_help_menu()
        clock.tick(60)

    elif game_running:
        if current_game_mode == "2player":
            run_game_2p()
        else:
            run_game()

    elif showing_stats:
        draw_statistics()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:  # Only left click
                mouse_pos = get_mouse_pos_logical()
                for rect, index in stat_tab_rects:  # type: ignore
                    if rect.collidepoint(mouse_pos):
                        if index == -1:
                            play_sound(click_back_sound)
                            showing_stats = False
                            in_profile_hub = True
                        elif isinstance(index, tuple):
                            # Sub-tab click inside Modded tab: (main_tab, sub_tab)
                            play_sound(click_sound)
                            modded_sub_tab = index[1]
                            stat_scroll = 0
                        else:
                            play_sound(click_sound)
                            current_stat_tab = index
                            stat_scroll = 0
            elif event.type == pygame.MOUSEWHEEL:
                adjust_stat_scroll(-event.y * 30)
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_TAB:
                    switch_stat_tab()
                elif event.key == pygame.K_m or is_back_key(event.key):
                    in_profile_hub = True
                    showing_stats = False
                else:
                    d = nav_direction(event.key)
                    _stat_nav_rects = [r for r, idx in stat_tab_rects if idx != -1]
                    _stat_nav_idxs  = [idx for r, idx in stat_tab_rects if idx != -1]
                    if d in ("left", "right"):
                        cur = nav_get_focus("statistics", len(_stat_nav_rects))
                        nav_set_focus("statistics", nav_move(_stat_nav_rects, cur, d))
                    elif d == "up":
                        adjust_stat_scroll(-30)
                    elif d == "down":
                        adjust_stat_scroll(30)
                    elif is_nav_confirm(event.key):
                        _fi = nav_get_focus("statistics", len(_stat_nav_rects))
                        if _fi is not None and _fi < len(_stat_nav_idxs):
                            _sidx = _stat_nav_idxs[_fi]
                            if isinstance(_sidx, tuple):
                                play_sound(click_sound)
                                modded_sub_tab = _sidx[1]
                                stat_scroll = 0
                            else:
                                play_sound(click_sound)
                                current_stat_tab = _sidx
                                stat_scroll = 0
        clock.tick(60)

    elif in_shop:
        shop_loop()

    elif in_loadout:
        loadout_loop()
    elif in_shop_menu:
        shop_menu_loop()
    elif in_cosmetic_shop:
        cosmetic_shop_loop()
    elif in_loadout_menu:
        loadout_menu_loop()
    elif in_cosmetic_inventory:
        cosmetic_inventory_loop()
    elif in_online_leaderboard:
        online_leaderboard_loop()
    elif in_player_search:
        player_search_loop()
    elif in_public_profile:
        public_profile_loop()
    elif in_friends_hub:
        friends_hub_loop()
    elif in_clan_hub:
        clan_hub_loop()
    elif in_clan_create:
        clan_create_loop()
    elif in_clan_search:
        clan_search_loop()
    elif in_clan_view:
        clan_view_loop()
    elif in_chat:
        chat_loop()
    elif in_rename_profile:
        rename_profile_loop()
    elif in_redeem_code:
        redeem_code_loop()

    elif in_admin_login:
        admin_login_loop()

    elif in_admin_panel:
        admin_panel_loop()

    elif in_loading:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
        draw_loading_screen()
        clock.tick(60)

    elif in_daily_rewards:
        can_claim, _ = can_claim_daily_reward()
        claim_rect = pygame.Rect(WIDTH//2 - 100, 150 + 50, 200, 50)
        back_rect = pygame.Rect(WIDTH - 180, HEIGHT - 60, 150, 40)
        _dr_rects = ([claim_rect] if can_claim else []) + [back_rect]
        _dr_actions = (["claim"] if can_claim else []) + ["back"]

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit(); sys.exit()
            _dr_action = None
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_m or is_back_key(event.key):
                    _dr_action = "back"
                else:
                    d = nav_direction(event.key)
                    if d:
                        cur = nav_get_focus("daily_rewards", len(_dr_rects))
                        nav_set_focus("daily_rewards", nav_move(_dr_rects, cur, d))
                    elif is_nav_confirm(event.key):
                        _idx = nav_get_focus("daily_rewards", len(_dr_rects))
                        if _idx is not None and _idx < len(_dr_actions):
                            _dr_action = _dr_actions[_idx]
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mouse_pos = get_mouse_pos_logical()
                mx, my = mouse_pos
                if can_claim and claim_rect.collidepoint((mx, my)):
                    _dr_action = "claim"
                elif back_rect.collidepoint((mx, my)):
                    _dr_action = "back"
            if _dr_action == "claim":
                play_sound(click_sound)
                claim_daily_reward_from_menu()
            elif _dr_action == "back":
                play_sound(click_back_sound)
                back_to_main_from_daily_rewards()
        draw_daily_rewards_menu()
        clock.tick(60)

