# The real Mixxx from Debian (2.5.0, schema 39) for tools/mixxx-acceptance/run.py --linux.
#
#   docker build -t cuerithm-mixxx-linux tools/mixxx-acceptance/linux
#
# Mixxx runs on a virtual display (Xvfb) with software OpenGL and no audio device, so nothing
# needs a screen or a click. libmad, FAAD and mp4v2 are the decoder libraries Mixxx uses on
# Linux, here for building the decoder probes (mad_probe.c).
FROM debian:trixie-slim
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      mixxx xvfb xauth sqlite3 gcc libc6-dev libmad0-dev libfaad-dev libmp4v2-dev \
      libgl1-mesa-dri libglx-mesa0 \
 && rm -rf /var/lib/apt/lists/*
