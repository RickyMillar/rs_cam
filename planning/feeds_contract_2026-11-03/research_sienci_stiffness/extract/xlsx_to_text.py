import sys, zipfile, re, xml.etree.ElementTree as ET
ns={'m':'http://schemas.openxmlformats.org/spreadsheetml/2006/main','r':'http://schemas.openxmlformats.org/officeDocument/2006/relationships'}
z=zipfile.ZipFile(sys.argv[1])
ss=[]
if 'xl/sharedStrings.xml' in z.namelist():
    for si in ET.fromstring(z.read('xl/sharedStrings.xml')).findall('m:si',ns):
        ss.append(''.join(t.text or '' for t in si.iter('{%s}t'%ns['m'])))
wb=ET.fromstring(z.read('xl/workbook.xml'))
rels=ET.fromstring(z.read('xl/_rels/workbook.xml.rels'))
rmap={r.get('Id'):r.get('Target') for r in rels}
for sh in wb.find('m:sheets',ns):
    rid=sh.get('{%s}id'%ns['r']); tgt=rmap[rid]; tgt=tgt.lstrip('/'); tgt=tgt if tgt.startswith('xl/') else 'xl/'+tgt
    print(f"=== sheet: {sh.get('name')} ===")
    root=ET.fromstring(z.read(tgt))
    for row in root.iter('{%s}row'%ns['m']):
        cells=[]
        for c in row.findall('m:c',ns):
            v=c.find('m:v',ns); f=c.find('m:f',ns); t=c.get('t')
            if v is None and c.find('m:is',ns) is None: continue
            if t=='s': val=ss[int(v.text)]
            elif t=='inlineStr': val=''.join(x.text or '' for x in c.iter('{%s}t'%ns['m']))
            else: val=v.text
            s=f"{c.get('r')}={val!r}"
            if f is not None and f.text: s+=f" [={f.text}]"
            cells.append(s)
        if cells: print('  '.join(cells))
