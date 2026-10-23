# Build and run environment for a Mixxx built from source (`main`, schema 40) with FAAD and libmad,
# for tools/mixxx-acceptance/run.py --linux-main. The package list is the noble branch of
# tools/debian_buildenv.sh in the Mixxx source, plus a virtual display to run it on. It leaves out
# libmp4v2, which FAAD needs and which no longer has a package, so build.sh builds it.
#
#   docker build -t cuerithm-mixxx-main-deps tools/mixxx-acceptance/linux-main
#   docker run --rm -v /path/to/mixxx:/src:ro -v mixxx-main-out:/out \
#       -v "$PWD/tools/mixxx-acceptance/linux-main:/ctx:ro" cuerithm-mixxx-main-deps sh /ctx/build.sh
#   python3 tools/mixxx-acceptance/run.py --linux-main mixxx-main-out
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential g++ cmake ninja-build pkg-config git ca-certificates mold sqlite3 \
      xvfb xauth libgl1-mesa-dri libglx-mesa0 fonts-open-sans fonts-ubuntu \
      libavformat-dev libavcodec-dev libavutil-dev libswresample-dev libchromaprint-dev \
      libebur128-dev libfaad-dev libfftw3-dev libflac-dev libgl1-mesa-dev libgtest-dev libgmock-dev \
      libhidapi-dev libid3tag0-dev liblilv-dev libmad0-dev libmodplug-dev libmp3lame-dev \
      libmsgsl-dev libopus-dev libopusfile-dev libpipewire-0.3-dev libportmidi-dev \
      libprotobuf-dev libqt6opengl6-dev libqt6sql6-sqlite libqt6svg6-dev librubberband-dev \
      libshout-idjc-dev libsndfile1-dev libsoundtouch-dev libspa-0.2-dev libsqlite3-dev \
      libssl-dev libtag1-dev libudev-dev libupower-glib-dev libusb-1.0-0-dev libwavpack-dev \
      lv2-dev markdown portaudio19-dev protobuf-compiler qtkeychain-qt6-dev \
      qt6-declarative-private-dev qt6-base-private-dev qt6-multimedia-dev qt6-shadertools-dev \
      qt6-5compat-dev qml6-module-qtcore qml6-module-qtquick-controls \
      qml6-module-qtquick-layouts qml6-module-qtquick-window qml6-module-qtquick-shapes \
      qml6-module-qtquick-templates qml6-module-qtqml-workerscript qml6-module-qtquick-dialogs \
      qml6-module-qt5compat-graphicaleffects qml6-module-qt-labs-qmlmodels \
      qml6-module-qt-labs-folderlistmodel qml6-module-qtmultimedia \
 && rm -rf /var/lib/apt/lists/*
