import sys, html.parser, re
class P(html.parser.HTMLParser):
    def __init__(s):
        super().__init__(); s.out=[]; s.skip=0
    def handle_starttag(s,t,a):
        if t in('script','style','noscript','svg','head'): s.skip+=1
        if t in('p','br','div','tr','li','h1','h2','h3','h4','h5','table','article','section'): s.out.append('\n')
        if t in('td','th'): s.out.append(' | ')
        if t in('s','del','strike'): s.out.append('~~')
        if t=='img':
            d=dict(a)
            if d.get('alt') or d.get('src'): s.out.append(f"\n[img alt={d.get('alt','')!r} src={d.get('src','')}]\n")
    def handle_endtag(s,t):
        if t in('script','style','noscript','svg','head'): s.skip=max(0,s.skip-1)
        if t in('p','tr','li','h1','h2','h3','h4','h5','table'): s.out.append('\n')
        if t in('s','del','strike'): s.out.append('~~')
    def handle_data(s,d):
        if not s.skip: s.out.append(d)
p=P(); p.feed(open(sys.argv[1],encoding='utf-8',errors='replace').read())
t=''.join(p.out); t=re.sub(r'[ \t]+',' ',t); t=re.sub(r'\n\s*\n+','\n\n',t)
print(t.strip())
