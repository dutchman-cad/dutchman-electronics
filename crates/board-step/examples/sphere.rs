//! Writes one sphere as a STEP file, for `scripts/check-board-step.sh`, which opens it in
//! SolidCraft and cadmpeg.
//!
//!   cargo run -p board-step --example sphere -- <out.step> <radius-mm>

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(out), Some(radius)) = (args.next(), args.next()) else {
        return Err("usage: sphere <out.step> <radius-mm>".into());
    };
    let radius: f64 = radius.parse().map_err(|e| format!("radius: {e}"))?;
    let step = board_step::sphere(radius, "sphere").map_err(|e| e.to_string())?;
    std::fs::write(&out, step.text).map_err(|e| format!("cannot write {out}: {e}"))
}
