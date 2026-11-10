import sys
p='src/game/state.rs'; s=open(p,encoding='utf-8').read()
marker='    // @@FELDER@@'
new=[]
for line in sys.stdin.read().strip().splitlines():
    line=line.strip()
    if not line: continue
    name=line.split(':')[0].strip()
    if f'pub {name}:' in s: continue
    new.append(f'    pub {line},')
s=s.replace(marker, '\n'.join(new)+'\n'+marker)
open(p,'w',encoding='utf-8').write(s)
print('added',len(new))
