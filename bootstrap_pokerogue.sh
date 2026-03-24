# This script requires Git, pnpm, and Node.js to be installed
# Usage: bootstrap_pokerogue.sh [--update]
#   --update  Pull latest changes if src-ext exists, or clone if it does not

UPDATE=0
if [ "$1" = "--update" ]; then
  UPDATE=1
fi

if [ -d "src-ext" ]; then
  if [ "$UPDATE" -eq 1 ]; then
    echo "src-ext exists, updating..."
    cd src-ext || exit
    if [ -n "$POKEROGUE_BRANCH" ]; then
      git fetch origin
      git checkout "$POKEROGUE_BRANCH"
      git pull origin "$POKEROGUE_BRANCH"
    else
      git pull
    fi
    cd ..
  else
    echo "src-ext exists, skipping clone."
  fi
else
  # Clone whether or not --update was supplied
  git clone --recurse-submodules -j8 https://github.com/pagefaultgames/pokerogue.git src-ext --depth 1 ${POKEROGUE_BRANCH:+--branch "$POKEROGUE_BRANCH"}
fi

cd src-ext || exit

pnpm install

# Append offline-mode env vars to .env (do not overwrite — preserve upstream defaults)
printf '\nVITE_BYPASS_LOGIN=1\nVITE_SERVER_URL=http://localhost:8001\n' >> .env

# Build in app mode: loads .env.app which sets VITE_SERVER_URL=http://localhost:8001,
# ensuring API calls go through the RogueTop proxy rather than the production server
pnpm build --mode app

# Compress dist folder to "game.dat"
cd dist || exit
zip -9 -q -r game.zip .
mv game.zip ../../game.dat

cd ../..
