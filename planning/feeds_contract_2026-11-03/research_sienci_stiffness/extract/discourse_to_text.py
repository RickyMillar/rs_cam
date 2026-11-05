import json,re,html,sys
src,out,url,pub=sys.argv[1:5]
d=json.load(open(src))
with open(out,'w') as f:
    f.write(f"Source: {url}\nFetched: 2026-11-06\nPublisher: {pub}\nExtraction: Discourse topic JSON ({src.split('/')[-1]}), HTML tags stripped\n----- BEGIN EXTRACTED TEXT -----\n")
    f.write(f"Title: {d['title']}\n")
    for p in d['post_stream']['posts']:
        t=html.unescape(re.sub('<[^>]+>','',p['cooked']))
        f.write(f"\n--- post {p['post_number']} by {p['username']} (staff={p.get('staff')}) {p['created_at']}\n{t}\n")
