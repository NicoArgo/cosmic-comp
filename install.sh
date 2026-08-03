#!/usr/bin/env bash
# Build and install POP Flow's cosmic-comp over the system compositor.
#
# ⚠ THIS IS THE COMPOSITOR. It draws your session. A build that fails to start
# leaves you at a login loop or a black screen, and a text console (Ctrl+Alt+F3)
# is the way back. Read ./uninstall.sh before running this — it is the recovery
# procedure, and it is written to work from a bare TTY.
#
# Nothing is restarted here: killing cosmic-comp would end your session on the
# spot, taking every unsaved thing with it. Log out and back in to pick up the
# new binary. Reversible with ./uninstall.sh
set -euo pipefail
cd "$(dirname "$0")"

echo "==> Building (cargo build --release)..."
echo "    This needs system libraries; if it fails on a missing .pc file, see"
echo "    Build-Depends in debian/control."
cargo build --release

BIN="target/release/cosmic-comp"
[ -f "$BIN" ] || { echo "Build failed: $BIN not found"; exit 1; }

# A compositor has to agree with the rest of the session — panel, settings
# daemon, workspaces, the session manager. Ours being far ahead of, or behind,
# the packages around it is the most likely way for this to go wrong, and it
# goes wrong at login, which is the worst moment.
SYSTEM_VER="$(dpkg-query -W -f='${Version}' cosmic-comp 2>/dev/null || true)"
if [ -n "$SYSTEM_VER" ]; then
    echo
    echo "==> System cosmic-comp package: $SYSTEM_VER"
    echo "    This build comes from $(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')."
    echo "    If your COSMIC packages are older than this source, upgrade them"
    echo "    first (sudo apt upgrade) so the session pieces match."
fi

echo
read -r -p "Install a new compositor? Type 'yes' to continue: " reply
[ "$reply" = "yes" ] || { echo "Aborted."; exit 1; }

if [ ! -f cosmic-comp.orig ] && [ -f /usr/bin/cosmic-comp ]; then
    echo "==> Backing up current /usr/bin/cosmic-comp -> ./cosmic-comp.orig"
    cp /usr/bin/cosmic-comp cosmic-comp.orig
fi
[ -f cosmic-comp.orig ] || { echo "Refusing to install with no backup to fall back to."; exit 1; }

echo "==> Installing to /usr/bin/cosmic-comp (needs sudo)..."
sudo install -m 0755 "$BIN" /usr/bin/cosmic-comp

GOLDEN=/usr/local/lib/pop-flow/cosmic-comp
if [ -f "$GOLDEN" ]; then
    echo "==> Refreshing auto-reapply golden copy"
    sudo install -m 0755 "$BIN" "$GOLDEN"
else
    echo "!! No auto-reapply hook installed: the next package update of"
    echo "   cosmic-comp will silently restore the stock compositor."
    echo "   Run ./setup-auto-reapply.sh to make this install stick."
fi

echo
echo "==> Done. NOTHING WAS RESTARTED — your session is still the old compositor."
echo "    Log out and back in when you are ready to try the new one."
echo
echo "    If the session does not come back:"
echo "      Ctrl+Alt+F3 -> log in -> cd $(pwd) -> ./uninstall.sh -> reboot"
