// CC0: original test board made for Dutchman Electronics.
// A non-rectangular board: an L shape, 40 x 30 mm with a 20 x 15 mm corner missing.
export default () => (
  <board
    outline={[
      { x: 0, y: 0 },
      { x: 40, y: 0 },
      { x: 40, y: 15 },
      { x: 20, y: 15 },
      { x: 20, y: 30 },
      { x: 0, y: 30 },
    ]}
  >
    <hole diameter="2mm" pcbX="10mm" pcbY="10mm" />
  </board>
)
