"""Regenerate this independently created fixture; flac is a preparation tool only."""
from pathlib import Path
import hashlib,struct,subprocess,tempfile,wave,json
P=Path(__file__).resolve().parent;output=P/'pulses64-stereo16.flac'
if output.exists():raise SystemExit('refusing to overwrite existing fixture')
seed=0x481aaa19;pcm=bytearray()
for i in range(64*44100):
 seed^=(seed<<13)&0xffffffff;seed^=seed>>17;seed^=(seed<<5)&0xffffffff
 phase=i%22050;value=(((seed&65535)-32768)*8*(4096-phase))//(10*4096)if phase<4096 else 0
 pcm.extend(struct.pack('<hh',value,value))
with tempfile.TemporaryDirectory()as directory:
 source=Path(directory)/'pulses.wav'
 with wave.open(str(source),'wb')as f:f.setnchannels(2);f.setsampwidth(2);f.setframerate(44100);f.writeframes(pcm)
 subprocess.run(['flac','--silent','-5','--blocksize=4096','--no-padding','-o',str(output),str(source)],check=True)
print(json.dumps({'frames':64*44100,'channels':2,'bits':16,'rate':44100,'pcm_sha256':hashlib.sha256(pcm).hexdigest(),'flac_sha256':hashlib.sha256(output.read_bytes()).hexdigest()}))
