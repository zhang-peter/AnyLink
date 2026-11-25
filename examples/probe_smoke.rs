use probe_rs::Permissions;
use probe_rs::config::{Registry, TargetSelector};
use probe_rs::probe::cmsisdap::CmsisDapFactory;
use probe_rs::probe::list::Lister;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("list") {
        let filter = std::env::args()
            .nth(2)
            .unwrap_or_else(|| "STM32".to_string());
        let registry = Registry::from_builtin_families();
        for target in registry.search_chips(&filter) {
            println!("{target}");
        }
        return Ok(());
    }

    let probes = Lister::new().list_all();
    if probes.is_empty() {
        return Err("no debug probes found".into());
    }

    for probe in &probes {
        println!(
            "{} | {:04x}:{:04x} | {} | serial={:?}",
            probe.identifier,
            probe.vendor_id,
            probe.product_id,
            probe.probe_type(),
            probe.serial_number
        );
    }

    let info = probes
        .into_iter()
        .find(|probe| probe.is_probe_type::<CmsisDapFactory>())
        .ok_or("no CMSIS-DAP probe found")?;

    let probe = info.clone().open()?;
    println!("opened CMSIS-DAP probe: {}", info.identifier);

    let selector = std::env::args()
        .nth(1)
        .map(TargetSelector::Unspecified)
        .unwrap_or(TargetSelector::Auto);
    println!("attaching with {:?}", selector);
    let session = probe.attach(selector, Permissions::default())?;

    println!("target: {}", session.target().name);
    println!("architecture: {:?}", session.architecture());
    println!("cores: {:?}", session.list_cores());
    Ok(())
}
