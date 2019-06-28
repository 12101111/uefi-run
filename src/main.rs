use std::path::Path;
use std::process::{Child, Command, exit};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wait_timeout::ChildExt;

fn main() {
    let matches = clap::App::new("uefi-run")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Richard Wiedenhöft <richard@wiedenhoeft.xyz>")
        .about("Runs UEFI executables in qemu.")
        .setting(clap::AppSettings::TrailingVarArg)
        .setting(clap::AppSettings::DontDelimitTrailingValues)
        .arg(
            clap::Arg::with_name("efi_exe")
                .value_name("FILE")
                .required(true)
                .help("EFI executable"),
        )
        .arg(
            clap::Arg::with_name("bios_path")
                .value_name("bios_path")
                .required(false)
                .help("BIOS image (default = /usr/share/OVMF/{OVMF.fd, x64/OVMF_CODE.fd} or ./OVMF.fd)")
                .short("b")
                .long("bios"),
        )
        .arg(
            clap::Arg::with_name("qemu_path")
                .value_name("qemu_path")
                .required(false)
                .help("Path to qemu executable (default = qemu-system-x86_64)")
                .short("q")
                .long("qemu"),
        )
        .arg(
            clap::Arg::with_name("qemu_args")
                .value_name("qemu_args")
                .required(false)
                .help("Additional arguments for qemu")
                .multiple(true),
        )
        .get_matches();

    // Parse options
    let efi_exe = matches.value_of("efi_exe").unwrap();
    let bios_path = matches.value_of("bios_path").unwrap_or_else(|| {
        // Debian Ubuntu
        if Path::new("/usr/share/OVMF/OVMF.fd").exists() {
            "/usr/share/OVMF/OVMF.fd"
        // Archlinux
        } else if Path::new("/usr/share/ovmf/x64/OVMF_CODE.fd").exists() {
            "/usr/share/ovmf/x64/OVMF_CODE.fd"
        } else if Path::new("OVMF.fd").exists() {
            "OVMF.fd"
        } else {
            eprintln!("Unable to find OVMF.fd");
            exit(1);
        }
    });
    dbg!(bios_path);
    let qemu_path = matches
        .value_of("qemu_path")
        .unwrap_or("qemu-system-x86_64");
    let user_qemu_args = matches
        .values_of("qemu_args")
        .unwrap_or(clap::Values::default());

    // Install termination signal handler. This ensures that the destructor of
    // `temp_dir` which is constructed in the next step is really called and
    // the files are cleaned up properly.
    let terminating = Arc::new(AtomicBool::new(false));
    {
        let term = terminating.clone();
        ctrlc::set_handler(move || {
            println!("uefi-run terminating...");
            // Tell the main thread to stop waiting.
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting termination handler");
    }

    // Create temporary dir for ESP.
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    // Path to /EFI/BOOT
    let efi_boot_path = temp_dir.path().join("EFI").join("BOOT");
    std::fs::create_dir_all(efi_boot_path.clone()).expect("Unable to create /EFI/BOOT directory");
    let bootx64_path = efi_boot_path.join("BOOTX64.EFI");
    std::fs::copy(efi_exe, bootx64_path).expect("Unable to copy EFI executable");

    let qemu_args_ref = vec![
        // Disable default devices.
        // QEMU by defaults enables a ton of devices which slow down boot.
        "-nodefaults",
        // Use a modern machine, with acceleration if possible.
        "-machine","q35,accel=kvm:tcg",
        // A standard VGA card with Bochs VBE extensions.
        "-vga","std",
        // Connect the serial port to the host. OVMF is kind enough to connect
        // the UEFI stdout and stdin to that port too.
        "-serial","stdio",
        // Set up OVMF.
        "-bios",bios_path,
        // Mount a local directory as a FAT partition.
        "-drive",
    ];
    let mut qemu_args:Vec<_> = qemu_args_ref.into_iter().map(|x| x.into()).collect();
    qemu_args.push(format!("format=raw,file=fat:rw:{}", temp_dir.path().display()));
    qemu_args.extend(user_qemu_args.map(|x| x.into()));

    // Run qemu.
    let mut child = Command::new(qemu_path)
        .args(qemu_args)
        .spawn()
        .expect("Failed to start qemu");

    // Wait for qemu to exit or signal.
    let mut child_terminated;
    loop {
        child_terminated = wait_qemu(&mut child, Duration::from_millis(500));
        if child_terminated || terminating.load(Ordering::SeqCst) {
            break;
        }
    }

    // If uefi-run received a signal we still need the child to exit.
    if !child_terminated {
        child_terminated = wait_qemu(&mut child, Duration::from_secs(1));
        if !child_terminated {
            match child.kill() {
                // Kill succeeded
                Ok(_) => assert!(wait_qemu(&mut child, Duration::from_secs(1))),
                Err(e) => {
                    match e.kind() {
                        // Not running anymore
                        std::io::ErrorKind::InvalidInput => {
                            assert!(wait_qemu(&mut child, Duration::from_secs(1)))
                        }
                        // Other error
                        _ => panic!("Not able to kill child process: {:?}", e),
                    }
                }
            }
        }
    }
}

/// Wait for the process to exit for `duration`.
///
/// Returns `true` if the process exited and false if the timeout expired.
fn wait_qemu(child: &mut Child, duration: Duration) -> bool {
    let wait_result = child
        .wait_timeout(duration)
        .expect("Failed to wait on child process");
    match wait_result {
        None => {
            // Child still alive.
            return false;
        }
        Some(exit_status) => {
            // Child exited.
            if !exit_status.success() {
                match exit_status.code() {
                    Some(code) => println!("qemu exited with status {}", code),
                    None => println!("qemu exited unsuccessfully"),
                }
            }
            return true;
        }
    }
}
