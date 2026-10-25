#!/bin/sh
# Runs inside the cuerithm-mixxx-main-deps container (see its Dockerfile): build mp4v2, which FAAD
# needs and no distro package provides any more, at a pinned commit, then Mixxx out of tree. The
# Mixxx source is mounted read-only at /src; everything written goes to /out. Engine DJ export
# (libdjinterop, fetched and built by Mixxx) is switched off: it does not build with GCC 13's
# -Werror=stringop-overflow and has nothing to do with this test.
set -e
git config --global --add safe.directory '*'

# mp4v2 v2.1.3 of the maintained fork (https://github.com/enzo1982/mp4v2), by commit
if [ ! -f /out/mp4v2/lib/libmp4v2.so ] && [ ! -f /out/mp4v2/lib/libmp4v2.a ]; then
  rm -rf /out/mp4v2-src
  git clone https://github.com/enzo1982/mp4v2 /out/mp4v2-src
  git -C /out/mp4v2-src checkout f4c2f1b87446100728472d80fd1161d2f8e830b2
  cmake -S /out/mp4v2-src -B /out/mp4v2-build -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/out/mp4v2 -DBUILD_SHARED=ON -DBUILD_UTILS=OFF
  cmake --build /out/mp4v2-build -j 4
  cmake --install /out/mp4v2-build
fi

cmake -S /src -B /out/build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/out/install \
  -DCMAKE_PREFIX_PATH=/out/mp4v2 -DQT6=ON -DQML=OFF -DFAAD=ON -DMAD=ON -DFFMPEG=ON \
  -DENGINEPRIME=OFF -DOPTIMIZE=portable -DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=mold
# moc output made before gtest was installed cannot expand FRIEND_TEST; make it again.
rm -rf /out/build/mixxx-lib_autogen
# Only the application: the unit tests stay on for moc (it needs FRIEND_TEST) but are not built.
cmake --build /out/build --target mixxx -j 4
cmake --install /out/build
echo BUILD-COMPLETE
