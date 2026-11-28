"""Loads a saved board, fills its zones and saves it again (KiCad 9 fills only from a loaded file)."""
import sys

import pcbnew

path = sys.argv[1]
b = pcbnew.LoadBoard(path)
b.BuildConnectivity()
zones = list(b.Zones())
for fp in b.GetFootprints():
    zones.extend(fp.Zones())  # zones inside footprints are filled too
pcbnew.ZONE_FILLER(b).Fill(zones)
b.Save(path)
