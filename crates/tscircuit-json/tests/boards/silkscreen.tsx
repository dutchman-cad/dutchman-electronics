// CC0: original test board made for Dutchman Electronics.
// Silkscreen text, a line, a rectangle and a circle on a 50 x 30 mm board.
export default () => (
  <board width="50mm" height="30mm">
    <silkscreentext text="DUTCHMAN" pcbX="0mm" pcbY="8mm" fontSize="2mm" />
    <silkscreenline x1="-20mm" y1="-10mm" x2="20mm" y2="-10mm" strokeWidth="0.2mm" />
    <silkscreenrect pcbX="-15mm" pcbY="0mm" width="6mm" height="4mm" strokeWidth="0.15mm" />
    <silkscreencircle pcbX="15mm" pcbY="0mm" radius="3mm" strokeWidth="0.15mm" />
  </board>
)
