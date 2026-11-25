import math, itertools
def R(ax,deg):
    c,s=math.cos(math.radians(deg)),math.sin(math.radians(deg))
    return {0:[[1,0,0],[0,c,-s],[0,s,c]],1:[[c,0,s],[0,1,0],[-s,0,c]],2:[[c,-s,0],[s,c,0],[0,0,1]]}[ax]
def mul(a,b): return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def axisangle(m):
    tr=m[0][0]+m[1][1]+m[2][2]
    ang=math.acos(max(-1,min(1,(tr-1)/2)))
    if abs(ang)<1e-9: return (0,0,1,0)
    if abs(ang-math.pi)<1e-6:
        # symmetric: axis from diagonal
        v=[math.sqrt(max(0,(m[i][i]+1)/2)) for i in range(3)]
        # fix signs using off-diagonals
        if v[0]>1e-6:
            v[1]=math.copysign(v[1],m[0][1]) if v[1]>1e-6 else 0
            v[2]=math.copysign(v[2],m[0][2]) if v[2]>1e-6 else 0
        elif v[1]>1e-6: v[2]=math.copysign(v[2],m[1][2])
        return tuple(round(x,3) for x in v)+(round(ang,3),)
    ax=((m[2][1]-m[1][2]),(m[0][2]-m[2][0]),(m[1][0]-m[0][1]))
    n=math.sqrt(sum(x*x for x in ax))
    return tuple(round(x/n,3) for x in ax)+(round(ang,3),)
cases={"rxy":(90,90,0),"ryx":(0,90,90),"rzx":(90,0,90)}
for name,(rx,ry,rz) in cases.items():
    a=(-rx,-ry,-rz)
    xfirst=mul(R(2,a[2]),mul(R(1,a[1]),R(0,a[0])))
    zfirst=mul(R(0,a[0]),mul(R(1,a[1]),R(2,a[2])))
    print(name,"X-first",axisangle(xfirst),"Z-first",axisangle(zfirst))
