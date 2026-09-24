#!/usr/bin/env python3
"""G2 fetch helper: strip an HTML page to plain text (no third-party deps).
Usage: g2_html2text.py in.html > out.txt"""
import sys, re, html
from html.parser import HTMLParser

class P(HTMLParser):
    def __init__(self):
        super().__init__(); self.out=[]; self.skip=0
    def handle_starttag(self, t, a):
        if t in ('script','style','noscript','svg'): self.skip+=1
        if t in ('br','p','div','tr','li','h1','h2','h3','h4','table','section'): self.out.append('\n')
        if t in ('td','th'): self.out.append(' | ')
    def handle_endtag(self, t):
        if t in ('script','style','noscript','svg') and self.skip: self.skip-=1
    def handle_data(self, d):
        if not self.skip: self.out.append(d)
p=P(); p.feed(open(sys.argv[1],encoding='utf-8',errors='replace').read())
t=''.join(p.out)
t=re.sub(r'[ \t\r]+',' ',t)
t=re.sub(r'\n\s*\n+','\n',t)
print(t.strip())
