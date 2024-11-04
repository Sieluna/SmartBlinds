fn main() {
    linker_message();

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let exe_path = std::env::current_exe().unwrap().display().to_string();

    if target_arch.starts_with("xtensa") {
        println!(
            "cargo:rustc-link-arg=-Wl,--error-handling-script={}",
            exe_path
        );
    } else if target_arch.starts_with("riscv") {
        println!(
            "cargo:rustc-link-arg=--error-handling-script={}",
            exe_path
        );
    }
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}

fn linker_message() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                "_defmt_timestamp" => {
                    eprintln!();
                    eprintln!("💡 `defmt` not found - make sure `defmt.x` is added as a linker script and you have included `use defmt_rtt as _;`");
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("💡 Is the linker script `linkall.x` missing?");
                    eprintln!();
                }
                "esp_wifi_preempt_enable"
                | "esp_wifi_preempt_yield_task"
                | "esp_wifi_preempt_task_create" => {
                    eprintln!();
                    eprintln!("💡 `esp-wifi` has no scheduler enabled. Make sure you have the `builtin-scheduler` feature enabled, or that you provide an external scheduler.");
                    eprintln!();
                }
                "embedded_test_linker_file_not_added_to_rustflags" => {
                    eprintln!();
                    eprintln!("💡 `embedded-test` not found - make sure `embedded-test.x` is added as a linker script for tests");
                    eprintln!();
                }
                _ => (),
            },
            // we don't have anything helpful for "missing-lib" yet
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }
}
