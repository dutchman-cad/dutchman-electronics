// CC0: original test board made for Dutchman Electronics.
// A four-layer 50 x 30 mm board with two resistors routed between them.
export default () => (
  <board width="50mm" height="30mm" layers={4}>
    <resistor name="R1" resistance="1k" footprint="0603" pcbX="-12mm" pcbY="0mm" />
    <resistor name="R2" resistance="1k" footprint="0603" pcbX="12mm" pcbY="0mm" />
    <trace from=".R1 > .pin2" to=".R2 > .pin1" />
  </board>
)
