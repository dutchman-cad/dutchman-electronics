// CC0: original test board made for Dutchman Electronics.
// A few common parts with 3D models: 0603 resistor, 0805 capacitor, SOIC-8 chip,
// a through-hole 4-pin header (plated holes) and an LED, on a 60 x 40 mm board.
export default () => (
  <board width="60mm" height="40mm">
    <resistor name="R1" resistance="10k" footprint="0603" pcbX="-20mm" pcbY="10mm" />
    <capacitor name="C1" capacitance="100nF" footprint="0805" pcbX="-20mm" pcbY="0mm" pcbRotation="90deg" />
    <chip name="U1" footprint="soic8" pcbX="0mm" pcbY="5mm" />
    <pinheader name="J1" pinCount={4} pitch="2.54mm" pcbX="15mm" pcbY="-10mm" />
    <led name="D1" footprint="0603" pcbX="20mm" pcbY="10mm" />
    <resistor name="R2" resistance="1k" footprint="0603" pcbX="0mm" pcbY="-12mm" layer="bottom" />
  </board>
)
