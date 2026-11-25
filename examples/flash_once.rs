use anylink::backend::{self, FlashEvent, FlashRequest};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let hex = args
        .next()
        .ok_or_else(|| "usage: flash_once <firmware.hex> [target]".to_string())?;
    let target = args.next().unwrap_or_else(|| "STM32H743ZI".to_string());

    let probes = backend::list_daplink_probes()?;
    let probe = probes
        .first()
        .ok_or_else(|| "no DAP-Link / CMSIS-DAP probe found".to_string())?;
    println!("probe={} [{}]", probe.name, probe.detail());

    let request = FlashRequest {
        probe: probe.key.clone(),
        target,
        hex_path: hex.into(),
        verify: true,
    };

    let mut last_operation = None;
    let mut last_percent = -1.0_f32;
    let outcome = backend::flash_hex(&request, false, &mut |event| match event {
        FlashEvent::Log(message) => println!("{message}"),
        FlashEvent::Progress(progress) => {
            if last_operation != Some(progress.operation) {
                last_operation = Some(progress.operation);
                last_percent = -1.0;
                println!("phase: {}", progress.operation.label());
            }
            let percent = progress.percent();
            if percent >= 99.0 && last_percent < 99.0 {
                println!("progress: {percent:.0}%");
            }
            last_percent = percent;
        }
    })?;

    println!(
        "SUCCESS target={} file={} size={} verified={} dry_run={} elapsed_ms={}",
        outcome.target,
        outcome.file.display(),
        outcome.file_size,
        outcome.verified,
        outcome.dry_run,
        outcome.elapsed.as_millis()
    );
    Ok(())
}
