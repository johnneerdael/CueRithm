#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/../.." && pwd)"
task_build="$task_root/.superpowers/media-build"
task_dest="$task_root/tools/media-runtime/bin"
mkdir -p "$task_build" "$task_dest"
python3 "$task_root/tools/media-runtime/prepare.py"
cd "$task_build/ffmpeg-9.0.2"
task_link_flags=""
case "$(uname -s)" in MINGW*|MSYS*) task_link_flags="-static";; esac
./configure --prefix="$task_dest" --disable-autodetect --disable-network \
    --disable-doc --disable-debug --disable-shared --enable-static \
    --disable-everything --enable-ffmpeg --enable-ffprobe \
    --enable-avcodec --enable-avformat --enable-avfilter --enable-swresample \
    --enable-protocol=file,pipe \
    --enable-demuxer=aac,aiff,asf,flac,mov,mp3,ogg,wav \
    --enable-parser=aac,aac_latm,flac,mpegaudio,opus,vorbis \
    --enable-decoder=aac,alac,flac,mp3float,mp3,opus,vorbis,wmav1,wmav2,wmapro,wmalossless,pcm_s8,pcm_u8,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be,pcm_f32le,pcm_f32be,pcm_f64le,pcm_f64be,pcm_alaw,pcm_mulaw \
    --enable-encoder=pcm_f32le --enable-muxer=pcm_f32le,image2pipe \
    --enable-filter=aresample,aformat,anull --extra-ldflags="$task_link_flags"
task_ext=""
case "$(uname -s)" in MINGW*|MSYS*) task_ext=".exe";; esac
make -j 8 "ffmpeg$task_ext" "ffprobe$task_ext"
cp "ffmpeg$task_ext" "$task_dest/ffmpeg$task_ext"
cp "ffprobe$task_ext" "$task_dest/ffprobe$task_ext"
cp COPYING.LGPLv2.1 "$task_dest/COPYING.LGPLv2.1"
python3 "$task_root/tools/media-runtime/seal.py" "$task_dest"
