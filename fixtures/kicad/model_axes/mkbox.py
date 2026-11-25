# A STEP box x 0..4, y 0..1, z 0..0.5 (own code, for measuring how KiCad places a model).
import sys
X,Y,Z=4.0,1.0,0.5
ents=[]
def add(s):
    ents.append(s); return len(ents)
def pt(x,y,z): return add(f"CARTESIAN_POINT('',({x},{y},{z}))")
def dr(x,y,z): return add(f"DIRECTION('',({x},{y},{z}))")
V=[(0,0,0),(X,0,0),(X,Y,0),(0,Y,0),(0,0,Z),(X,0,Z),(X,Y,Z),(0,Y,Z)]
P=[pt(*v) for v in V]
VP=[add(f"VERTEX_POINT('',#{p})") for p in P]
edges=[(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)]
EC=[]
for a,b in edges:
    d=[V[b][i]-V[a][i] for i in range(3)]
    L=sum(x*x for x in d)**.5
    dd=dr(*[x/L for x in d])
    vec=add(f"VECTOR('',#{dd},{L})")
    ln=add(f"LINE('',#{P[a]},#{vec})")
    EC.append(add(f"EDGE_CURVE('',#{VP[a]},#{VP[b]},#{ln},.T.)"))
def oe(i,fwd): return add(f"ORIENTED_EDGE('',*,*,#{EC[i]},.{'T' if fwd else 'F'}.)")
# faces: (edge index,forward) loops, plane origin, normal, refdir
faces=[
 ([(3,0),(2,0),(1,0),(0,0)],(0,0,0),(0,0,-1),(1,0,0)),
 ([(4,1),(5,1),(6,1),(7,1)],(0,0,Z),(0,0,1),(1,0,0)),
 ([(0,1),(9,1),(4,0),(8,0)],(0,0,0),(0,-1,0),(1,0,0)),
 ([(1,1),(10,1),(5,0),(9,0)],(X,0,0),(1,0,0),(0,1,0)),
 ([(2,1),(11,1),(6,0),(10,0)],(0,Y,0),(0,1,0),(-1,0,0)),
 ([(3,1),(8,1),(7,0),(11,0)],(0,0,0),(-1,0,0),(0,-1,0)),
]
FA=[]
for loop,o,n,r in faces:
    oes=[oe(i,f) for i,f in loop]
    el=add("EDGE_LOOP('',("+",".join(f"#{x}" for x in oes)+"))")
    fb=add(f"FACE_OUTER_BOUND('',#{el},.T.)")
    po=pt(*o); pn=dr(*n); pr=dr(*r)
    ax=add(f"AXIS2_PLACEMENT_3D('',#{po},#{pn},#{pr})")
    pl=add(f"PLANE('',#{ax})")
    FA.append(add(f"ADVANCED_FACE('',(#{fb}),#{pl},.T.)"))
sh=add("CLOSED_SHELL('',("+",".join(f"#{x}" for x in FA)+"))")
br=add(f"MANIFOLD_SOLID_BREP('box',#{sh})")
o=pt(0,0,0); z=dr(0,0,1); x=dr(1,0,0)
ax=add(f"AXIS2_PLACEMENT_3D('',#{o},#{z},#{x})")
app=add("APPLICATION_CONTEXT('automotive design')")
add(f"APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})")
pc=add(f"PRODUCT_CONTEXT('',#{app},'mechanical')")
prod=add(f"PRODUCT('box','box','',(#{pc}))")
pdf=add(f"PRODUCT_DEFINITION_FORMATION('','',#{prod})")
dc=add(f"PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')")
pd=add(f"PRODUCT_DEFINITION('design','',#{pdf},#{dc})")
pds=add(f"PRODUCT_DEFINITION_SHAPE('','',#{pd})")
lu=add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))")
au=add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))")
su=add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())")
un=add("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),#%d,'distance_accuracy_value','')"%lu)
ctx=add(f"(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{un}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{lu},#{au},#{su}))REPRESENTATION_CONTEXT('',''))")
sr=add(f"ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{ax},#{br}),#{ctx})")
add(f"SHAPE_DEFINITION_REPRESENTATION(#{pds},#{sr})")
out="ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('box'),'2;1');\nFILE_NAME('box.step','2026-01-01T00:00:00',(''),(''),'','','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n"
out+="".join(f"#{i+1}={e};\n" for i,e in enumerate(ents))+"ENDSEC;\nEND-ISO10303-21;\n"
open(sys.argv[1],"w").write(out)
