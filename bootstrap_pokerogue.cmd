@REM This script requires Git, pnpm, 7zip, and Node.js to be installed
@REM Usage: bootstrap_pokerogue.cmd [--update]
@REM   --update  Pull latest changes if src-ext exists, or clone if it does not

set UPDATE=0
if "%~1"=="--update" set UPDATE=1

if exist "src-ext\" (
    if "%UPDATE%"=="1" (
        echo src-ext exists, updating...
        cd src-ext
        if defined POKEROGUE_BRANCH (
            git fetch origin
            git checkout "%POKEROGUE_BRANCH%"
            git pull origin "%POKEROGUE_BRANCH%"
        ) else (
            git pull
        )
        cd ..
    ) else (
        echo src-ext exists, skipping clone.
    )
) else (
    if defined POKEROGUE_BRANCH (
        git clone --recurse-submodules -j8 --depth 1 --branch "%POKEROGUE_BRANCH%" https://github.com/pagefaultgames/pokerogue.git src-ext
    ) else (
        git clone --recurse-submodules -j8 --depth 1 https://github.com/pagefaultgames/pokerogue.git src-ext
    )
)

cd src-ext

pnpm install

@REM Set offline-mode vars (idempotent: remove stale values first, then append)
if exist .env (
    findstr /v /c:"VITE_BYPASS_LOGIN=" /c:"VITE_SERVER_URL=" .env > .env.tmp 2>nul
    move /y .env.tmp .env >nul
)
echo. >> .env
echo VITE_BYPASS_LOGIN=1 >> .env
echo VITE_SERVER_URL=http://localhost:8001 >> .env

@REM Build in app mode: loads .env.app which sets VITE_SERVER_URL=http://localhost:8001,
@REM ensuring API calls go through the RogueTop proxy rather than the production server
pnpm build --mode app

@REM Compress dist folder to "game.dat"
cd dist
7z a -tzip -mx9 -r ../../game.dat *

cd ../..
