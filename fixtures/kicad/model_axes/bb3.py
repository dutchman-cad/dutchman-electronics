import re,sys
for f in sys.argv[1:]:
    t=open(f).read()
    tr=[float(x) for x in re.search(r'translation ([-\d.e]+) ([-\d.e]+) ([-\d.e]+)',t.split('TXFM_2')[1]).groups()]
    shapes=re.split(r'DEF SHAPE_\d+ Shape',t)[1:7]
    pts=[]
    for s in shapes:
        m=re.search(r'Coordinate \{ point \[(.*?)\]',s,re.S)
        for trip in m.group(1).replace('\n','').split(','):
            v=[float(x) for x in trip.split()]
            if len(v)==3: pts.append(v)
    rel=lambda p:[round(p[i]*2.54,2) for i in range(3)]
    lo=[min(p[i] for p in pts)*2.54 for i in range(3)]; hi=[max(p[i] for p in pts)*2.54 for i in range(3)]
    print(f,"extent x %.2f..%.2f y %.2f..%.2f z %.2f..%.2f"%(lo[0],hi[0],lo[1],hi[1],lo[2],hi[2]), "n",len(pts))
