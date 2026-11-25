"""Loads a saved board, fills its zones and saves it again (KiCad 9 fills only from a loaded file)."""
import sys

import pcbnew

path = sys.argv[1]
b = pcbnew.LoadBoard(path)
b.BuildConnectivity()
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(path)
