use nih_plug::prelude::*;

use vst_js::Gain;

fn main() {
    nih_export_standalone::<Gain>();
}
