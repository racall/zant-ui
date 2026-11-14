//! `zant-math`: the math formula example. The application lives in
//! [`zant_examples::math_app`] so the headless smoke test can build the same
//! tree the window paints.

fn main() -> zant::Result<()> {
    zant::ui::run(
        zant_examples::math_app::MathApp,
        zant_examples::math_app::run_options(),
    )
}
