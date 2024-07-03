// Build script

use copy_to_output::copy_to_output;

fn main() {
    // Re-run script if any files under res/ changed:
    println!("cargo:rerun-if-changed=res/*");
    copy_to_output("res", "")
        .expect("Could not copy resource files to output target directory.");
}

