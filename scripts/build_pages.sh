#!/usr/bin/env bash
# Build the static GitHub Pages copy of the desktop into site/.
#
#   scripts/frontend.sh &                 # live panel, for recording
#   python3 scripts/record_replay.py      # real session -> site/replay/
#   scripts/build_pages.sh                # site/ = desktop + replay + results
#
# The page plays back the recorded kernel session (frontend/replay.js); it
# never pretends to be live. Publish site/ on the gh-pages branch.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SITE="$ROOT/site"
[ -f "$SITE/replay/session.json" ] || { echo "no recording: run scripts/record_replay.py first" >&2; exit 1; }

find "$SITE" -mindepth 1 -maxdepth 1 ! -name replay -exec rm -rf {} +
cp -r "$ROOT/frontend/static/." "$SITE/"
cp "$ROOT/frontend/replay.js" "$SITE/replay.js"
mkdir -p "$SITE/results"
cp "$ROOT"/results/*.png "$ROOT"/results/*.md "$SITE/results/" 2>/dev/null || true

# Pages serves the site under /micro-os/, so absolute /static/ and /results/
# paths become relative.
for f in "$SITE"/*.html "$SITE"/*.js "$SITE"/*.css; do
    sed -i -e 's#"/static/#"#g' -e "s#'/static/#'#g" -e 's#url(/static/#url(#g' -e 's#(\"/static/#("#g' \
           -e 's#"/results/#"results/#g' -e 's#`/results/#`results/#g' -e "s#'/results/#'results/#g" "$f"
done

# Load the replay shim before the desktop's own script.
python3 - "$SITE/index.html" <<'PY'
import re, sys
p = sys.argv[1]; s = open(p).read()
s = re.sub(r'(<script[^>]*src="app\.js[^"]*"[^>]*></script>)', r'<script src="replay.js"></script>\n\1', s, count=1)
assert 'src="replay.js"' in s, "app.js script tag not found"
open(p, "w").write(s)
PY
touch "$SITE/.nojekyll"
echo "built $SITE ($(du -sh "$SITE" | cut -f1))"
