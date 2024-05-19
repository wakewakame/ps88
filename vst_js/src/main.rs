use nih_plug::prelude::*;

use vst_js::VstJs;

fn main() {
    nih_export_standalone::<VstJs>();
}
