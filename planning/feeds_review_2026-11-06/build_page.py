"""Build index.html from page_src.html and claims.json. Usage: python3 -I build_page.py
{{id}} becomes <span data-claim="id">shown</span>; {{CLAIMS_JSON}} becomes the claims block."""
import json, os, re
HERE = os.path.dirname(os.path.abspath(__file__))
C = json.load(open(os.path.join(HERE, "claims.json")))
src = open(os.path.join(HERE, "page_src.html"), encoding="utf-8").read()
src = src.replace("{{CLAIMS_JSON}}", json.dumps(C).replace("</", "<\\/"))
def sub(m):
    cid = m.group(1)
    return f'<span data-claim="{cid}">{C[cid]["shown"]}</span>'
out = re.sub(r"\{\{([a-z0-9_]+)\}\}", sub, src)
open(os.path.join(HERE, "index.html"), "w", encoding="utf-8").write(out)
print("index.html", len(out))
