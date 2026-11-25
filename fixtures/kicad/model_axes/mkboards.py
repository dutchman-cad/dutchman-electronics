import pcbnew
from pcbnew import FromMM, VECTOR2I, VECTOR3D
cases={"off":((3,2,1),(0,0,0)),"rz":((0,0,0),(0,0,90)),"rx":((0,0,0),(90,0,0)),"ry":((0,0,0),(0,90,0)),"rxy":((0,0,0),(90,90,0)),"ryx":((0,0,0),(0,90,90)),"rzx":((0,0,0),(90,0,90))}
for flip in (False,True):
  for name,(off,rot) in cases.items():
    b=pcbnew.CreateEmptyBoard()
    fp=pcbnew.FOOTPRINT(b); fp.SetReference("U1")
    m=pcbnew.FP_3DMODEL(); m.m_Filename="/work/box.step"
    m.m_Offset=VECTOR3D(*off); m.m_Rotation=VECTOR3D(*rot); m.m_Scale=VECTOR3D(1,1,1)
    fp.Models().append(m)
    b.Add(fp); fp.SetPosition(VECTOR2I(FromMM(100),FromMM(100)))
    if flip: fp.Flip(fp.GetPosition(), False)
    b.Save(f"/work/{'b' if flip else 't'}_{name}.kicad_pcb")
