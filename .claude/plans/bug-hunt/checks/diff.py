import json,re,sys,os
snap=open(sys.argv[1]).read()
outdir=sys.argv[2]
exp={}
for m in re.finditer(r'exports\[`(\w+) file (.+?) 1`\] = `(.*?)`;\n',snap,re.S):
    body=m.group(3).replace('\\`','`').replace('\\\\','\\')
    try: exp[m.group(2)]=json.loads(body)
    except Exception as e: print('SNAPERR',m.group(2),e)
def key(mv):
    if mv is None: return None
    f=mv.get('from'); 
    return (mv.get('color'), (f['x'],f['y']) if f else None, (mv['to']['x'],mv['to']['y']), mv.get('piece'), bool(mv.get('promote')), mv.get('capture'), bool(mv.get('same')))
def walk(ms,path,out):
    for i,m in enumerate(ms):
        out.append((path+(i,), key(m.get('move')), m.get('special'), len(m.get('forks') or []), tuple(m.get('comments') or [])))
        for k,fk in enumerate(m.get('forks') or []): walk(fk,path+(i,'f',k),out)
    return out
summary={}
for name,e in sorted(exp.items()):
    p=os.path.join(outdir,name+'.json')
    if not os.path.exists(p): print(f'MISSING\t{name}'); continue
    g=json.load(open(p))
    issues=[]
    ei=(e.get('initial') or {}).get('preset','HIRATE'); gi=(g.get('initial') or {}).get('preset','HIRATE')
    if ei!=gi: issues.append(f'preset {ei}!={gi}')
    if (e.get('initial') or {}).get('data') != (g.get('initial') or {}).get('data') and ei=='OTHER':
        ed=e['initial']['data']; gd=g['initial']['data']
        if ed.get('color')!=gd.get('color'): issues.append('initial color')
        if ed.get('hands')!=gd.get('hands'): issues.append(f"hands {ed.get('hands')} vs {gd.get('hands')}")
        if json.dumps(ed.get('board'))!=json.dumps(gd.get('board')): issues.append('board differs')
    eh=e.get('header',{}); gh=g.get('header',{})
    for k in set(eh)|set(gh):
        if eh.get(k)!=gh.get(k): issues.append(f'header[{k}] {eh.get(k)!r} vs {gh.get(k)!r}'[:120])
    E=walk(e['moves'],(),[]); G=walk(g['moves'],(),[])
    if len(E)!=len(G): issues.append(f'node count {len(E)} vs {len(G)}')
    for a,b in zip(E,G):
        if a[0]!=b[0]: issues.append(f'path {a[0]} vs {b[0]}'); break
        for idx,label in [(1,'move'),(2,'special'),(3,'forks'),(4,'comments')]:
            if a[idx]!=b[idx]:
                issues.append(f'{label}@{a[0]}: {a[idx]} vs {b[idx]}'[:200]); 
    print(('OK' if not issues else 'DIFF')+'\t'+name+('\t'+str(len(issues))+'\t'+' | '.join(issues[:3]) if issues else ''))
