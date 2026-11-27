// CC0: original test board made for Dutchman Electronics.
// 50 x 30 mm board, one 3 mm hole at (-15, 0), one 10 x 10 mm cutout centred at (10, 0).
export default () => (
  <board width="50mm" height="30mm">
    <hole diameter="3mm" pcbX="-15mm" pcbY="0mm" />
    <cutout shape="rect" width="10mm" height="10mm" pcbX="10mm" pcbY="0mm" />
  </board>
)
