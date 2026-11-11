#!/bin/bash
# Multiplayer Ende-zu-Ende-Test (Phase 5): echter Server + zwei unsichtbare Spiel-Instanzen.
# Spieler A erstellt einen VS-Raum (Hölle), Spieler B tritt über die Raumliste bei, beide
# werden bereit, die Runde läuft bis zum Ende, dann Rematch und zweite Runde.
# Nutzung: tests/mp_e2e.sh [ausgabeordner]   (braucht einen Debug-Build und unter Linux ein X-Display, z.B. xvfb-run)
set -u
cd "$(dirname "$0")/.."
OUT=${1:-$(mktemp -d)}
mkdir -p "$OUT"
BIN=target/debug/JumpNDodge
SRV=target/debug/JumpNDodge-Server
PORT=$((47900 + RANDOM % 500))
SD=$(mktemp -d); A=$(mktemp -d); B=$(mktemp -d)
echo "{\"name\":\"E2E-Testserver\",\"port\":$PORT,\"log_file\":\"\"}" > "$SD/server.json"
$SRV --config "$SD/server.json" > "$OUT/server.log" 2>&1 &
SPID=$!
sleep 0.5
common="10:cmd:run multiplayer;20:cmd:run mp connect 127.0.0.1:$PORT"
JND_HIDDEN=1 JND_MUTE=1 JND_BASE_DIR=$A JND_SCRIPT="$common;60:cmd:run mp create vs Hölle;150:shot:$OUT/a_room.png;160:cmd:run mp ready;400:shot:$OUT/a_round.png;2400:shot:$OUT/a_result.png;2410:cmd:run mp rematch;2600:shot:$OUT/a_round2.png;5200:quit" \
  timeout 150 $BIN > "$OUT/a.log" 2>&1 &
APID=$!
JND_HIDDEN=1 JND_MUTE=1 JND_BASE_DIR=$B JND_SCRIPT="$common;60:cmd:run mp list;120:cmd:run mp join first;165:cmd:run mp ready;400:shot:$OUT/b_round.png;2410:cmd:run mp rematch;5200:quit" \
  timeout 150 $BIN > "$OUT/b.log" 2>&1 &
BPID=$!
wait $APID; AE=$?
wait $BPID; BE=$?
kill $SPID 2>/dev/null; wait $SPID 2>/dev/null
rm -rf "$SD" "$A" "$B"
fail=0
check() { if grep -q "$2" "$OUT/$1"; then echo "OK   $1: $2"; else echo "FEHLT $1: $2"; fail=1; fi; }
for c in a.log b.log; do
  check $c "\[MP\] verbunden mit E2E-Testserver"
  check $c "\[MP\] Raum "
  check $c "\[MP\] Runde gestartet"
  check $c "\[MP\] Runde vorbei"
done
check a.log "du = P1"
check b.log "du = P2"
n=$(grep -c "\[MP\] Runde gestartet" "$OUT/a.log")
[ "$n" -ge 2 ] && echo "OK   Rematch: $n Runden" || { echo "FEHLT Rematch (nur $n Runde)"; fail=1; }
# beide müssen dasselbe Ergebnis sehen
ra=$(grep "\[MP\] Runde vorbei" "$OUT/a.log" | head -1 | sed 's/.*Sieger//'); rb=$(grep "\[MP\] Runde vorbei" "$OUT/b.log" | head -1 | sed 's/.*Sieger//')
[ -n "$ra" ] && [ "$ra" = "$rb" ] && echo "OK   gleiches Ergebnis:$ra" || { echo "FEHLT gleiches Ergebnis ('$ra' / '$rb')"; fail=1; }
grep -q "panicked" "$OUT/a.log" "$OUT/b.log" && { echo "FEHLER: Absturz"; fail=1; }
[ $AE -eq 0 ] && [ $BE -eq 0 ] || { echo "FEHLER: Exit-Codes $AE/$BE"; fail=1; }
echo "Ausgabe: $OUT"
exit $fail
