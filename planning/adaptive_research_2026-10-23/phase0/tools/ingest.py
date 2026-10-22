"""Copy one saved MCP tool reply (raw JSON text or a [{type,text}] list) to a clean JSON file.

Usage: python3 ingest.py <saved reply> <out.json>
"""
import json
import sys


def load(path):
    text = open(path).read()
    data = json.loads(text)
    if isinstance(data, list) and data and isinstance(data[0], dict) and "text" in data[0]:
        data = json.loads(data[0]["text"])
    return data


if __name__ == "__main__":
    d = load(sys.argv[1])
    json.dump(d, open(sys.argv[2], "w"))
    print(sys.argv[2], d.get("spans_total_matching"), d.get("spans_returned"))
