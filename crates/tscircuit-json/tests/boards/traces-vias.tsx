// CC0: original test board made for Dutchman Electronics.
// Two resistors joined by a trace, one via at (0, 8), on a 50 x 30 mm two-layer board.
export default () => (
  <board width="50mm" height="30mm">
    <resistor name="R1" resistance="1k" footprint="0603" pcbX="-12mm" pcbY="0mm" />
    <resistor name="R2" resistance="1k" footprint="0603" pcbX="12mm" pcbY="0mm" />
    <trace from=".R1 > .pin2" to=".R2 > .pin1" />
    <via pcbX="0mm" pcbY="8mm" fromLayer="top" toLayer="bottom" holeDiameter="0.3mm" outerDiameter="0.6mm" />
  </board>
)
