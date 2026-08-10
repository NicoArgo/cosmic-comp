#!/usr/bin/env bash
# Restore the original system cosmic-comp (undo install.sh).
#
# Written and kept working BEFORE install.sh is ever run: this is the compositor,
# and if a bad build fails to start you are looking at a login loop or a black
# screen, with a TTY as the only way back in. Recovering must not depend on
# anything but this script.
#
# TO RECOVER FROM A BROKEN COMPOSITOR:
#   1. Ctrl+Alt+F3   (a text console; F3 through F6 usually work)
#   2. log in
#   3. cd to this directory and run:  ./uninstall.sh
#   4. reboot, or Ctrl+Alt+F1/F2 to return to the graphical session
set -euo pipefail
cd "$(dirname "$0")"

[ -f cosmic-comp.orig ] || { echo "No backup (cosmic-comp.orig) found."; exit 1; }

# Turn off auto-reapply first, or the next package operation would re-patch the
# binary right after we restore the original. Delegating to the script that owns
# those paths rather than repeating them: this used to remove only the golden
# copy, leaving a root-owned APT hook behind for good.
if [ -x ./remove-auto-reapply.sh ]; then
    ./remove-auto-reapply.sh
fi

echo "==> Restoring original /usr/bin/cosmic-comp (needs sudo)..."
sudo install -m 0755 cosmic-comp.orig /usr/bin/cosmic-comp

echo
echo "==> Restored."
echo "    The running compositor is still the POP Flow one — it is only replaced"
echo "    on disk. LOG OUT AND BACK IN (or reboot) to run the original again."
