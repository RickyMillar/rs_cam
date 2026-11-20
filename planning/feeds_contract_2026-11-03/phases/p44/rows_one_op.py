import json, sys
# usage: rows20.py <out dir a> <out dir b> <toolpath id>: each limit row whose value, bound or state differs; other differing keys by name
a=json.load(open(sys.argv[1]+"/summary.json")); b=json.load(open(sys.argv[2]+"/summary.json")); i=int(sys.argv[3])
ta=[t for t in a["per_toolpath"] if t["id"]==i][0]; tb=[t for t in b["per_toolpath"] if t["id"]==i][0]
n=0; other=set()
def flat(d,pre=""):
    out={}
    for k,v in (d.items() if isinstance(d,dict) else enumerate(d)):
        if isinstance(v,(dict,list)): out.update(flat(v,pre+str(k)+"."))
        else: out[pre+str(k)]=v
    return out
for x,y in zip(ta["limit_rows"],tb["limit_rows"]):
    cx,cy=x["check"],y["check"]; key=(cx["limit"],x["subject"].get("group"))
    if (cx["value"],cx["bound"],cx["state"])!=(cy["value"],cy["bound"],cy["state"]):
        n+=1; print(key,(cx["value"],cx["bound"],cx["state"]["state"]),"->",(cy["value"],cy["bound"],cy["state"]["state"]))
    fx,fy=flat(x),flat(y)
    for k in set(fx)|set(fy):
        if fx.get(k)!=fy.get(k) and not k.startswith(("check.value","check.bound","check.state")): other.add((key,k,fx.get(k),fy.get(k)))
print("rows", len(ta["limit_rows"]), len(tb["limit_rows"]), "rows with another value, bound or state:", n)
for o in sorted(other, key=str)[:12]: print("  other:", o)
