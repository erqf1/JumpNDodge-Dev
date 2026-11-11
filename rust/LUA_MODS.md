# Lua-Mods für JumpNDodge (Rust Edition)

Mods sind einfache Textdateien mit der Endung `.lua` im Ordner **`mods`** neben deinem Spielstand
(Windows/portabel: neben dem Spiel; Linux-Paket: `~/.local/share/JumpNDodge/mods`;
macOS: `~/Library/Application Support/JumpNDodge/mods`).

Am schnellsten geht's im Spiel: **Hauptmenü → Mods (unten links) → Beispiel-Mod anlegen**, dann
**Ordner öffnen** und `beispiel.lua` mit einem Texteditor (z.B. Textline) ändern, **Neu laden** drücken.

## Sicherheit

- Jede Mod läuft in einer eigenen Sandbox (Lua 5.4) mit nur `table`, `string`, `math` und `utf8`.
  Es gibt **keinen** Zugriff auf Dateien, Netzwerk, Programme oder den Spielstand.
- Speicherlimit 16 MB, Zeitlimit pro Aufruf (Endlosschleifen werden abgebrochen).
- Fehler schalten nur die betroffene Mod ab – das Mods-Menü zeigt die Fehlermeldung.
- **Runden mit aktiven Mods zählen getrennt** (eigene Highscores, keine echten Coins) und
  nicht für die Online-Rangliste – wie bei den Mods der Python-Version.

## Aufbau einer Mod

```lua
mod = {
  name = "Goldregen",          -- Anzeige im Mods-Menü
  author = "Du",
  version = "1.0",
  description = "Kurzbeschreibung",
}
```

### Eigene Hindernisse

```lua
jnd.obstacle{
  id = "gold",                 -- Name, um es per on_tick zu erzeugen
  w = 30, h = 30,              -- Größe (10–200)
  color = {240, 200, 60},      -- Farbe (R, G, B)
  weight = 25,                 -- wie oft es statt eines normalen Hindernisses kommt (normal = 100; 0 = nie automatisch)
  speed = 1.3,                 -- Tempo-Faktor (0.2–4)
  top = false,                 -- true = fällt von oben
}
```

### Spielwerte ändern

```lua
jnd.modifier{ gravity = 0.9, jump = 1.1, speed = 1.0, lives = 1 }
-- gravity/jump/speed sind Faktoren (1 = normal), lives = zusätzliche Leben
```

### Eigener Modus

Erscheint im Mods-Menü als „Spielen: Name“.

```lua
jnd.mode{
  id = "sprint", name = "Sprint", description = "Schnell, nur ein Leben.",
  speed = 13,                  -- Hindernis-Tempo (Mittel = 8, Hölle = 32)
  min_delay = 350, max_delay = 800,   -- Abstand zwischen Hindernissen in ms
  lives = 1,
  top = true,                  -- Hindernisse von oben
}
```

### Ereignisse

```lua
function on_round_start(info)        -- info.difficulty, info.width, info.height, info.mode
end

function on_tick(state)              -- 60x pro Sekunde: state.tick, state.score, state.lives,
                                     -- state.player.x, state.player.y, state.width, state.height
  if state.tick % 300 == 0 then
    return { spawn = "gold" }        -- oder { spawn = {"gold", "gold"} }
  end
end

function on_hit(state)               -- state.score, state.lives
  jnd.log("Treffer!")                -- erscheint im Mods-Menü (auch print funktioniert)
end
```

Halte `on_tick` kurz – es läuft 60-mal pro Sekunde und hat nur wenige Millisekunden Zeit.
