use probe_rs::probe::cmsisdap::CmsisDapFactory;
use probe_rs::probe::list::Lister;
use probe_rs::{MemoryInterface, Permissions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let target = args.next().unwrap_or_else(|| "STM32H743ZI".to_string());
    let address = parse_number(&args.next().unwrap_or_else(|| "0x08000000".to_string()))?;
    let length = parse_number(&args.next().unwrap_or_else(|| "16".to_string()))? as usize;

    let probes = Lister::new().list_all();
    let info = probes
        .into_iter()
        .find(|probe| probe.is_probe_type::<CmsisDapFactory>())
        .ok_or("no CMSIS-DAP probe found")?;
    let probe = info.open()?;
    let mut session = probe.attach(target.as_str(), Permissions::default())?;
    let attached_target = session.target().name.clone();
    let mut core = session.core(0)?;

    let mut data = vec![0_u8; length];
    core.read_8(address, &mut data)?;

    println!("target={attached_target} address=0x{address:08X} bytes={length}");
    for (offset, chunk) in data.chunks(16).enumerate() {
        let hex = chunk
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("0x{:08X}: {hex}", address + (offset * 16) as u64);
    }
    Ok(())
}

fn parse_number(value: &str) -> Result<u64, std::num::ParseIntError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse()
    }
}
