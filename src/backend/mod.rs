use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use probe_rs::Permissions;
use probe_rs::config::Registry;
use probe_rs::flashing::{DownloadOptions, FlashProgress, HexLoader, build_loader};
use probe_rs::probe::cmsisdap::CmsisDapFactory;
use probe_rs::probe::list::Lister;
use probe_rs::probe::{DebugProbeInfo, Probe};
use serde::{Deserialize, Serialize};

/// A stable identifier for a debug probe. USB enumeration order is not stable,
/// so selections are persisted by VID/PID, serial number and interface number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeKey {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub interface: Option<u8>,
}

impl ProbeKey {
    fn from_info(info: &DebugProbeInfo) -> Self {
        Self {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            serial_number: info.serial_number.clone(),
            interface: info.interface,
        }
    }

    fn matches(&self, info: &DebugProbeInfo) -> bool {
        if self.vendor_id != info.vendor_id
            || self.product_id != info.product_id
            || self.interface != info.interface
        {
            return false;
        }

        match (&self.serial_number, &info.serial_number) {
            (Some(expected), Some(actual)) => expected.eq_ignore_ascii_case(actual),
            (None, None) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeDescriptor {
    pub key: ProbeKey,
    pub name: String,
    pub probe_type: String,
}

impl ProbeDescriptor {
    pub fn detail(&self) -> String {
        let serial = self.key.serial_number.as_deref().unwrap_or("无序列号");
        format!(
            "{:04X}:{:04X} · {} · {}",
            self.key.vendor_id, self.key.product_id, serial, self.probe_type
        )
    }
}

#[derive(Clone, Debug)]
pub struct FlashRequest {
    pub probe: ProbeKey,
    pub target: String,
    pub hex_path: PathBuf,
    pub verify: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashOperation {
    Fill,
    Erase,
    Program,
    Verify,
}

impl FlashOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fill => "准备 Flash 数据",
            Self::Erase => "擦除 Flash",
            Self::Program => "写入 Flash",
            Self::Verify => "校验 Flash",
        }
    }
}

impl From<probe_rs::flashing::ProgressOperation> for FlashOperation {
    fn from(value: probe_rs::flashing::ProgressOperation) -> Self {
        match value {
            probe_rs::flashing::ProgressOperation::Fill => Self::Fill,
            probe_rs::flashing::ProgressOperation::Erase => Self::Erase,
            probe_rs::flashing::ProgressOperation::Program => Self::Program,
            probe_rs::flashing::ProgressOperation::Verify => Self::Verify,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlashProgressUpdate {
    pub operation: FlashOperation,
    pub current: u64,
    pub total: Option<u64>,
}

impl FlashProgressUpdate {
    pub fn percent(&self) -> f32 {
        match self.total {
            Some(total) if total > 0 => {
                ((self.current.min(total) as f64 / total as f64) * 100.0) as f32
            }
            _ => 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FlashEvent {
    Log(String),
    Progress(FlashProgressUpdate),
}

#[derive(Clone, Debug)]
pub struct FlashOutcome {
    pub target: String,
    pub file: PathBuf,
    pub file_size: u64,
    pub verified: bool,
    pub elapsed: Duration,
    pub dry_run: bool,
}

#[derive(Clone, Debug)]
pub struct TargetInfo {
    pub target: String,
    pub architecture: String,
    pub cores: String,
}

pub fn list_daplink_probes() -> Result<Vec<ProbeDescriptor>, String> {
    let probes = list_raw_probes()?;
    Ok(probes
        .iter()
        .filter(|info| info.is_probe_type::<CmsisDapFactory>())
        .map(|info| ProbeDescriptor {
            key: ProbeKey::from_info(info),
            name: info.identifier.clone(),
            probe_type: info.probe_type(),
        })
        .collect())
}

fn list_raw_probes() -> Result<Vec<DebugProbeInfo>, String> {
    std::panic::catch_unwind(|| Lister::new().list_all()).map_err(|_| {
        "枚举 USB 调试器时发生内部错误。请拔出并重新连接 DAP-Link 后重试。".to_string()
    })
}

fn open_selected_probe(key: &ProbeKey) -> Result<Probe, String> {
    let probes = list_raw_probes()?;
    let info = probes
        .into_iter()
        .filter(|info| info.is_probe_type::<CmsisDapFactory>())
        .find(|info| key.matches(info))
        .ok_or_else(|| {
            "未找到之前选择的 DAP-Link。请确认设备已连接，并点击“刷新设备”。".to_string()
        })?;

    info.open().map_err(|error| {
        format!("无法打开 DAP-Link：{error}\n请关闭其他调试/下载工具，并重新插拔调试器。")
    })
}

fn resolve_target(target: &str) -> Result<String, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("请选择或输入目标芯片型号。".to_string());
    }

    let registry = Registry::from_builtin_families();
    let resolved = registry.get_target_by_name(target).map_err(|error| {
        format!(
            "无法识别目标芯片“{target}”：{error}\n请在芯片搜索框中输入完整型号，例如 STM32H743ZI。"
        )
    })?;
    Ok(resolved.name)
}

fn attach(probe: Probe, target: &str) -> Result<probe_rs::Session, String> {
    probe
        .attach(target, Permissions::default())
        .map_err(|error| {
            format!(
                "连接目标芯片“{target}”失败：{error}\n请检查 SWD 接线、目标板供电以及所选芯片型号。"
            )
        })
}

/// Attach to a target without flashing anything. This is used by the hardware
/// self-check and is safe with respect to the firmware currently on the chip.
pub fn identify_target(key: &ProbeKey, target: &str) -> Result<TargetInfo, String> {
    let target = resolve_target(target)?;
    let probe = open_selected_probe(key)?;
    let session = attach(probe, &target)?;

    Ok(TargetInfo {
        target: session.target().name.clone(),
        architecture: format!("{:?}", session.architecture()),
        cores: format!("{:?}", session.list_cores()),
    })
}

pub fn all_target_names() -> Vec<String> {
    let registry = Registry::from_builtin_families();
    let mut names = BTreeSet::new();
    for family in registry.families() {
        for chip in &family.variants {
            for package in chip.package_variants() {
                names.insert(package.clone());
            }
        }
    }
    names.into_iter().collect()
}

/// Return up to `limit` useful target-name suggestions. Prefix matches rank
/// before substring matches, and the ordering inside each group is stable.
pub fn suggest_targets(catalog: &[String], query: &str, limit: usize) -> Vec<String> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut prefix_matches = Vec::new();
    let mut contains_matches = Vec::new();
    for name in catalog {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with(&query) {
            prefix_matches.push(name.clone());
        } else if lower.contains(&query) {
            contains_matches.push(name.clone());
        }
    }

    prefix_matches.extend(contains_matches);
    prefix_matches.truncate(limit);
    prefix_matches
}

pub fn validate_hex_path(path: &Path) -> Result<u64, String> {
    let extension = path.extension().and_then(|ext| ext.to_str());
    if !extension.is_some_and(|ext| ext.eq_ignore_ascii_case("hex")) {
        return Err("仅支持 Intel HEX 文件（扩展名 .hex）。".to_string());
    }

    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("无法读取 HEX 文件“{}”：{error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("所选路径不是文件：{}", path.display()));
    }
    if metadata.len() == 0 {
        return Err("HEX 文件为空。".to_string());
    }

    Ok(metadata.len())
}

/// Flash an Intel HEX file. `emit` receives log lines and progress updates.
/// `dry_run` parses and lays out the image without erasing or programming.
pub fn flash_hex(
    request: &FlashRequest,
    dry_run: bool,
    emit: &mut impl FnMut(FlashEvent),
) -> Result<FlashOutcome, String> {
    let started = Instant::now();
    let file_size = validate_hex_path(&request.hex_path)?;
    let target = resolve_target(&request.target)?;

    emit(FlashEvent::Log(format!(
        "HEX 文件：{}（{} 字节）",
        request.hex_path.display(),
        file_size
    )));
    emit(FlashEvent::Log(format!("目标芯片：{target}")));

    let probe = open_selected_probe(&request.probe)?;
    emit(FlashEvent::Log(format!(
        "已打开 DAP-Link（{:04X}:{:04X}）",
        request.probe.vendor_id, request.probe.product_id
    )));

    let mut session = attach(probe, &target)?;
    let attached_target = session.target().name.clone();
    emit(FlashEvent::Log(format!(
        "已连接目标：{attached_target} / {:?}",
        session.architecture()
    )));

    let loader = build_loader(&mut session, &request.hex_path, HexLoader, None)
        .map_err(|error| format!("解析 HEX 失败：{error}"))?;
    let nvm_ranges: Vec<Range<u64>> = session
        .target()
        .memory_map
        .iter()
        .filter_map(|region| region.as_nvm_region())
        .map(|region| region.range.clone())
        .collect();
    let flash_bytes = overlapping_bytes(
        loader.data().map(|(address, data)| (address, data.len())),
        &nvm_ranges,
    );
    if flash_bytes == 0 {
        return Err(
            "HEX 中没有数据落在目标 Flash 区域，已停止操作。\n请检查 HEX 是否包含正确的扩展线性地址记录，例如 STM32 常见的 :020000040800F2。"
                .to_string(),
        );
    }
    emit(FlashEvent::Log(format!(
        "HEX 中有 {flash_bytes} 字节位于目标 Flash"
    )));

    emit(FlashEvent::Log(if dry_run {
        "开始 DRY RUN：只解析固件并检查 Flash 布局，不擦除、不写入。".to_string()
    } else {
        "开始下载固件。".to_string()
    }));

    let download_result = {
        let mut options = DownloadOptions::default();
        options.verify = request.verify && !dry_run;
        options.do_chip_erase = false;
        options.keep_unwritten_bytes = false;
        options.dry_run = dry_run;

        let mut active_operation: Option<FlashOperation> = None;
        let mut current = 0_u64;
        let mut total = None;

        options.progress = FlashProgress::new(|event| match event {
            probe_rs::flashing::ProgressEvent::FlashLayoutReady { flash_layout } => {
                let sectors: usize = flash_layout
                    .iter()
                    .map(|layout| layout.sectors().len())
                    .sum();
                let pages: usize = flash_layout.iter().map(|layout| layout.pages().len()).sum();
                emit(FlashEvent::Log(format!(
                    "固件布局已就绪：{sectors} 个扇区，{pages} 个页"
                )));
            }
            probe_rs::flashing::ProgressEvent::AddProgressBar {
                operation,
                total: operation_total,
            } => {
                active_operation = Some(operation.into());
                current = 0;
                total = operation_total;
                if let Some(operation) = active_operation {
                    emit(FlashEvent::Progress(FlashProgressUpdate {
                        operation,
                        current,
                        total,
                    }));
                }
            }
            probe_rs::flashing::ProgressEvent::Started(operation) => {
                active_operation = Some(operation.into());
                current = 0;
                if let Some(operation) = active_operation {
                    emit(FlashEvent::Progress(FlashProgressUpdate {
                        operation,
                        current,
                        total,
                    }));
                }
            }
            probe_rs::flashing::ProgressEvent::Progress {
                operation,
                size,
                time: _,
            } => {
                let operation: FlashOperation = operation.into();
                if active_operation != Some(operation) {
                    active_operation = Some(operation);
                    current = 0;
                    total = None;
                }
                current = current.saturating_add(size);
                emit(FlashEvent::Progress(FlashProgressUpdate {
                    operation,
                    current,
                    total,
                }));
            }
            probe_rs::flashing::ProgressEvent::Finished(operation) => {
                let operation: FlashOperation = operation.into();
                active_operation = Some(operation);
                if let Some(total) = total {
                    current = total;
                }
                emit(FlashEvent::Progress(FlashProgressUpdate {
                    operation,
                    current,
                    total,
                }));
            }
            probe_rs::flashing::ProgressEvent::Failed(operation) => {
                if !dry_run {
                    emit(FlashEvent::Log(format!(
                        "{}操作报告失败。",
                        FlashOperation::from(operation).label()
                    )));
                }
            }
            probe_rs::flashing::ProgressEvent::DiagnosticMessage { message } => {
                emit(FlashEvent::Log(format!("Flash 算法：{message}")));
            }
        });

        loader.commit(&mut session, options)
    };

    download_result.map_err(|error| {
        format!(
            "下载失败：{error}\n请检查 HEX 文件是否完整、芯片型号是否正确、下载地址是否在 Flash 范围内。"
        )
    })?;

    let verified = request.verify && !dry_run;
    emit(FlashEvent::Log(if dry_run {
        "DRY RUN 完成，未修改目标 Flash。".to_string()
    } else if verified {
        "固件写入并校验完成。".to_string()
    } else {
        "固件写入完成（未启用校验）。".to_string()
    }));

    Ok(FlashOutcome {
        target: attached_target,
        file: request.hex_path.clone(),
        file_size,
        verified,
        elapsed: started.elapsed(),
        dry_run,
    })
}

fn overlapping_bytes(chunks: impl Iterator<Item = (u64, usize)>, nvm_ranges: &[Range<u64>]) -> u64 {
    chunks
        .map(|(address, length)| {
            let end = address.saturating_add(length as u64);
            nvm_ranges
                .iter()
                .map(|range| {
                    let start = address.max(range.start);
                    let end = end.min(range.end);
                    end.saturating_sub(start)
                })
                .sum::<u64>()
        })
        .sum()
}

impl fmt::Display for FlashOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_prefer_prefix_and_respect_limit() {
        let catalog = vec![
            "STM32F103C8".to_string(),
            "STM32H743ZI".to_string(),
            "NUCLEO-STM32H743ZI".to_string(),
            "STM32H743VI".to_string(),
        ];

        assert_eq!(
            suggest_targets(&catalog, "STM32H743", 2),
            vec!["STM32H743ZI".to_string(), "STM32H743VI".to_string()]
        );
        assert_eq!(
            suggest_targets(&catalog, "h743zi", 10),
            vec!["STM32H743ZI".to_string(), "NUCLEO-STM32H743ZI".to_string()]
        );
    }

    #[test]
    fn suggestions_are_empty_without_a_query() {
        let catalog = vec!["STM32H743ZI".to_string()];
        assert!(suggest_targets(&catalog, "", 10).is_empty());
        assert!(suggest_targets(&catalog, "   ", 10).is_empty());
    }

    #[test]
    fn rejects_non_hex_and_missing_files() {
        let non_hex = Path::new("firmware.bin");
        assert!(validate_hex_path(non_hex).unwrap_err().contains(".hex"));

        let missing = Path::new("definitely-missing-firmware.hex");
        assert!(validate_hex_path(missing).unwrap_err().contains("无法读取"));
    }

    #[test]
    fn counts_only_bytes_inside_nvm_regions() {
        let nvm = vec![0x0800_0000..0x0810_0000, 0x0810_0000..0x0820_0000];
        let chunks = vec![
            (0x0000_0000, 16),
            (0x0800_0000, 8),
            (0x080F_FFFC, 8),
            (0x081F_FFFF, 1),
            (0x0820_0000, 16),
        ];
        assert_eq!(overlapping_bytes(chunks.into_iter(), &nvm), 17);
    }

    #[test]
    fn zero_overlap_means_the_hex_is_not_flashable() {
        let nvm = vec![0x0800_0000..0x0810_0000, 0x0810_0000..0x0820_0000];
        let chunks = vec![(0x0000_0000, 258)];
        assert_eq!(overlapping_bytes(chunks.into_iter(), &nvm), 0);
    }

    #[test]
    fn progress_percent_is_clamped() {
        let update = FlashProgressUpdate {
            operation: FlashOperation::Program,
            current: 150,
            total: Some(100),
        };
        assert_eq!(update.percent(), 100.0);

        let unknown_total = FlashProgressUpdate {
            operation: FlashOperation::Program,
            current: 50,
            total: None,
        };
        assert_eq!(unknown_total.percent(), 0.0);
    }
}
