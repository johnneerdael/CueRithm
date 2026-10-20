"""Turn a mitmproxy flow file of your own licensed copy's traffic into the dataset folder the research code reads.

    mitmdump -nr capture.flow -s import_capture.py --set out_dir=DIR

It runs offline over the saved flows (nothing is sent anywhere), keeps only the analysis calls, replaces every value under
a credential-looking key by "[REDACTED]" and keeps every other number exactly as received. The raw flow file still holds
credentials from the other requests: keep it private and out of the repository.

Writes `<flow id>.request.json` and `<flow id>.response.json` for each analysis call that got a JSON response, plus
`import-summary.json` with counts only (no titles, paths or labels).
"""
import json
import os
import re
from collections import Counter
from pathlib import Path
from urllib.parse import urlsplit

from mitmproxy import ctx

SECRET = re.compile(r"vipcode|authorization|cookie|token|password|secret|licen[cs]e|apikey|session|email|username", re.I)
STRING = re.compile(r'"(?:[^"\\]|\\.)*"')
ANALYSIS_PATH = re.compile(r"/Services/Analysis/AnalyzeSong\d+")
ANALYSIS_HOST = "online5.mixedinkey.com"


def redact_json(text):
    """Replace sensitive values without rounding or reserializing other numbers."""
    decoder = json.JSONDecoder()
    decoder.decode(text)  # Reject non-JSON rather than saving arbitrary content.
    spans = []
    pos = 0
    while (match := STRING.search(text, pos)) is not None:
        pos = match.end()
        rest = pos
        while rest < len(text) and text[rest].isspace():
            rest += 1
        if rest >= len(text) or text[rest] != ":":
            continue
        key = json.loads(match.group())
        if not SECRET.search(re.sub(r"[^a-z]", "", key.lower())):
            continue
        start = rest + 1
        while text[start].isspace():
            start += 1
        _, end = decoder.raw_decode(text, start)
        spans.append((start, end))
        pos = end
    for start, end in reversed(spans):
        text = text[:start] + '"[REDACTED]"' + text[end:]
    return text


def is_analysis(flow):
    hosts = {flow.request.host, flow.server_conn.sni, flow.client_conn.sni}
    return ANALYSIS_HOST in hosts and ANALYSIS_PATH.fullmatch(urlsplit(flow.request.path).path) is not None


class ImportCapture:
    def __init__(self):
        self.counts = Counter()
        self.genres = Counter()
        self.other_hosts = Counter()

    def load(self, loader):
        loader.add_option("out_dir", str, "", "Folder that receives the request/response pairs")

    def running(self):
        if not ctx.options.out_dir:
            raise ValueError("pass --set out_dir=DIR")
        self.out = Path(ctx.options.out_dir)
        self.out.mkdir(parents=True, exist_ok=True)

    def response(self, flow):
        self.counts["flows"] += 1
        if flow.response is None or not is_analysis(flow):
            self.other_hosts[flow.request.host] += 1
            return
        self.counts["analysis_calls"] += 1
        try:
            request = redact_json(flow.request.get_text(strict=True))
            response = redact_json(flow.response.get_text(strict=True))
        except (ValueError, UnicodeError):
            self.counts["not_json"] += 1
            return
        body = json.loads(response)
        if body.get("Error") or not body.get("Segments"):
            self.counts["error_or_empty_response"] += 1
        stem = self.out / flow.id
        if stem.with_suffix(".request.json").exists():
            self.counts["already_present"] += 1
            return
        for suffix, text in (("request.json", request), ("response.json", response)):
            with open(f"{stem}.{suffix}", "x", encoding="utf-8") as output:
                os.chmod(output.name, 0o600)
                output.write(text)
        self.counts["pairs_written"] += 1
        genre = (json.loads(request).get("EliteData") or {}).get("Genre") or "(none)"
        self.genres[genre] += 1

    def done(self):
        summary = {**self.counts, "genres": dict(self.genres.most_common()), "other_hosts": dict(self.other_hosts)}
        (self.out / "import-summary.json").write_text(json.dumps(summary, indent=2))
        ctx.log.info(json.dumps(summary))


addons = [ImportCapture()]
