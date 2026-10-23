"""Explore the app: python3 dump.py STEP... where STEP is click:RX | key:K | wait:S | waitfor:RX | gone:RX | set:RX=VALUE | scroll:RX,dir,pages | depth:N"""
import sys, time
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from cu import Cu
cu = Cu()
mindepth = 3
for step in sys.argv[1:]:
    k, _, v = step.partition(":")
    if k == "click": cu.click(v); time.sleep(1.0)
    elif k == "key": cu.key(v); time.sleep(0.6)
    elif k == "wait": time.sleep(float(v))
    elif k == "waitfor": cu.wait_for(v, 90)
    elif k == "gone": cu.wait_for(v, 120, absent=True)
    elif k == "set":
        rx, _, val = v.partition("=")
        e = cu.find(rx); cu.call("set_value", app="CueRithm", element_index=e["idx"], value=val); time.sleep(0.8)
    elif k == "scroll":
        rx, d, p = v.split(","); e = cu.find(rx); cu.call("scroll", app="CueRithm", element_index=e["idx"], direction=d, pages=float(p)); time.sleep(0.6)
    elif k == "depth": mindepth = int(v)
els = cu.state()
for e in els:
    if e["depth"] >= mindepth:
        print("  " * (e["depth"] - mindepth) + f'{e["idx"]} ' + e["text"].replace("\n", " | ")[:130])
cu.close()
