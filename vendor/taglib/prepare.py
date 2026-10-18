#!/usr/bin/env python3
"""Select reproducible build sources from the pinned official release archive."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
ORIGINAL='3ca2d8afaa7f1cf7f6ed10e511ebc368bfacd6dcaa3dbfa690b89e502e8963dc'
source=Path(sys.argv[1]);destination=Path(sys.argv[2])
assert hashlib.sha256(source.read_bytes()).hexdigest()==ORIGINAL,'Official source checksum mismatch'
audio={'.mp3','.flac','.m4a','.wma','.wav','.ogg','.opus','.aiff','.aac','.mpc','.ape','.wv','.aif','.tta','.spx','.dsf','.dff','.it','.mod','.s3m','.xm'}
with tarfile.open(source) as original, destination.open('wb') as output:
 with gzip.GzipFile(filename='',mode='wb',fileobj=output,mtime=0) as gz:
  with tarfile.open(fileobj=gz,mode='w',format=tarfile.PAX_FORMAT) as selected:
   for item in original:
    path=Path(item.name)
    if any(p in {'tests','examples','bench','.github'} for p in path.parts) or path.suffix.lower() in audio:continue
    item.uid=item.gid=item.mtime=0;item.uname=item.gname='';item.pax_headers={}
    selected.addfile(item,original.extractfile(item) if item.isfile() else None)
print(hashlib.sha256(destination.read_bytes()).hexdigest())
