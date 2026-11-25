use anylink::backend::{self, EraseRequest, FlashEvent, FlashRequest};
use anylink::{APP_NAME, APP_VERSION};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--version" | "-V") => {
            println!("{APP_NAME} {APP_VERSION}");
        }
        Some("--check") => {
            let target = args.get(2).map(String::as_str).unwrap_or("STM32H743ZI");
            if let Err(error) = run_hardware_check(target) {
                eprintln!("硬件检查失败：{error}");
                std::process::exit(1);
            }
        }
        Some("--dry-run") => {
            let Some(hex) = args.get(2) else {
                eprintln!("用法：anylink-cli --dry-run <firmware.hex> [target]");
                std::process::exit(2);
            };
            let target = args.get(3).map(String::as_str).unwrap_or("STM32H743ZI");
            if let Err(error) = run_flash_dry_run(hex, target) {
                eprintln!("DRY RUN 失败：{error}");
                std::process::exit(1);
            }
        }
        Some("--validate-hex") => {
            let Some(hex) = args.get(2) else {
                eprintln!("用法：anylink-cli --validate-hex <firmware.hex>");
                std::process::exit(2);
            };
            if let Err(error) = run_hex_validation(hex) {
                eprintln!("HEX 校验失败：{error}");
                std::process::exit(1);
            }
        }
        Some("--erase-chip") => {
            let target = args.get(2).map(String::as_str).unwrap_or("STM32H743ZI");
            if let Err(error) = run_chip_erase(target) {
                eprintln!("整片擦除失败：{error}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("AnyLink CLI {APP_VERSION}");
            eprintln!("用法：");
            eprintln!("  anylink-cli --check [target]");
            eprintln!("  anylink-cli --dry-run <firmware.hex> [target]");
            eprintln!("  anylink-cli --validate-hex <firmware.hex>");
            eprintln!("  anylink-cli --erase-chip [target]");
            eprintln!("  anylink-cli --version");
            std::process::exit(2);
        }
    }
}

fn run_chip_erase(target: &str) -> Result<(), String> {
    println!("AnyLink 整片擦除（仅擦除，不下载）");
    let probes = backend::list_supported_probes()?;
    let probe = probes
        .first()
        .ok_or_else(|| "未检测到 DAP-Link / ST-Link".to_string())?;
    println!("调试器：{} [{}]", probe.name, probe.detail());
    println!("目标芯片：{target}");

    let request = EraseRequest {
        probe: probe.key.clone(),
        target: target.to_string(),
    };
    let mut emit = |event| {
        if let FlashEvent::Log(message) = event {
            println!("{message}");
        }
    };
    let outcome = backend::erase_chip(&request, &mut emit)?;
    println!(
        "整片擦除完成：{} · {:.2}s",
        outcome.target,
        outcome.elapsed.as_secs_f64()
    );
    Ok(())
}

fn run_hex_validation(hex: &str) -> Result<(), String> {
    let info = backend::inspect_hex_file(hex.as_ref())?;
    println!("HEX 文件：{hex}");
    println!("文件大小：{} 字节", info.file_size);
    println!("数据记录：{} 条", info.data_records);
    println!("数据字节：{} 字节", info.data_bytes);
    match (info.start_address, info.end_address) {
        (Some(start), Some(end)) => {
            println!("地址范围：{start:#010X}–{end:#010X}");
        }
        _ => println!("地址范围：未知"),
    }
    println!("HEX 校验通过。");
    Ok(())
}

fn run_hardware_check(target: &str) -> Result<(), String> {
    println!("AnyLink 硬件检查（只读，不擦写 Flash）");
    let probes = backend::list_supported_probes()?;
    if probes.is_empty() {
        return Err("未检测到 DAP-Link / ST-Link".to_string());
    }

    for probe in &probes {
        println!("调试器：{} [{}]", probe.name, probe.detail());
    }

    let probe = probes
        .first()
        .ok_or_else(|| "未检测到 DAP-Link / ST-Link".to_string())?;
    println!("连接目标：{target}");
    let info = backend::identify_target(&probe.key, target)?;
    println!("目标：{}", info.target);
    println!("架构：{}", info.architecture);
    println!("核心：{}", info.cores);
    println!("硬件检查通过，未修改目标 Flash。");
    Ok(())
}

fn run_flash_dry_run(hex: &str, target: &str) -> Result<(), String> {
    println!("AnyLink Flash DRY RUN（不擦除、不写入）");
    let probes = backend::list_supported_probes()?;
    let probe = probes
        .first()
        .ok_or_else(|| "未检测到 DAP-Link / ST-Link".to_string())?;
    println!("调试器：{} [{}]", probe.name, probe.detail());

    let request = FlashRequest {
        probe: probe.key.clone(),
        target: target.to_string(),
        hex_path: hex.into(),
        verify: true,
        chip_erase: false,
        reset_after_flash: false,
    };
    let mut report_progress = |event| match event {
        FlashEvent::Log(message) => println!("{message}"),
        FlashEvent::Progress(progress) => {
            if progress.operation == backend::FlashOperation::Program && progress.percent() >= 99.0
            {
                println!("写入布局检查完成");
            }
        }
    };
    let outcome = backend::flash_hex(&request, true, &mut report_progress)?;
    println!(
        "DRY RUN 通过：{} · {} 字节 · {:.2}s · 未修改 Flash",
        outcome.target,
        outcome.file_size,
        outcome.elapsed.as_secs_f64()
    );
    Ok(())
}
