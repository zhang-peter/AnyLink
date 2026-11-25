use std::path::PathBuf;

use anylink::APP_VERSION;
use anylink::backend::{
    self, FlashEvent, FlashOperation, FlashOutcome, FlashRequest, ProbeDescriptor, ProbeKey,
};
use anylink::config::{self, AppConfig};
use rfd::FileDialog;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    progress::Progress,
    scroll::ScrollableElement as _,
    searchable_list::SearchableVec,
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

enum UiEvent {
    Probes(Result<Vec<ProbeDescriptor>, String>),
    Targets(Vec<String>),
    Flash(FlashEvent),
    FlashFinished(Result<FlashOutcome, String>),
}

pub struct AnyLinkApp {
    probes: Vec<ProbeDescriptor>,
    selected_probe: Option<ProbeKey>,
    probes_loading: bool,
    probe_message: String,
    target_catalog: Vec<String>,
    target_catalog_loading: bool,
    target_select: Entity<SelectState<SearchableVec<SharedString>>>,
    hex_path: Option<PathBuf>,
    choosing_file: bool,
    verify: bool,
    busy: bool,
    progress_percent: f32,
    progress_total: Option<u64>,
    progress_label: String,
    logs: Vec<String>,
    logs_open: bool,
    result: Option<Result<String, String>>,
    config: AppConfig,
    sender: async_channel::Sender<UiEvent>,
    receiver: async_channel::Receiver<UiEvent>,
    event_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl AnyLinkApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = config::load_config();
        let target_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<SharedString>::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });

        let (sender, receiver) = async_channel::unbounded();
        let mut app = Self {
            probes: Vec::new(),
            selected_probe: None,
            probes_loading: false,
            probe_message: "尚未检测调试器".to_string(),
            target_catalog: Vec::new(),
            target_catalog_loading: false,
            target_select,
            hex_path: config.last_hex.clone(),
            choosing_file: false,
            verify: config.verify,
            busy: false,
            progress_percent: 0.0,
            progress_total: None,
            progress_label: "准备中".to_string(),
            logs: vec!["AnyLink 已启动，正在初始化……".to_string()],
            logs_open: false,
            result: None,
            config,
            sender,
            receiver,
            event_task: None,
            _subscriptions: Vec::new(),
        };

        let subscription = cx.subscribe_in(
            &app.target_select,
            window,
            |this, _state, event, _window, cx| {
                let SelectEvent::Confirm(value) = event;
                this.config.last_target =
                    value.as_ref().map(ToString::to_string).unwrap_or_default();
                if let Err(error) = config::save_config(&this.config) {
                    this.logs.push(format!("保存配置失败：{error}"));
                }
                cx.notify();
            },
        );
        app._subscriptions.push(subscription);
        app
    }

    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_event_loop(window, cx);
        self.refresh_probes(cx);
        self.load_target_catalog(cx);
        cx.notify();
    }

    fn start_event_loop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = self.receiver.clone();
        self.event_task = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                if this
                    .update_in(cx, |this, window, cx| this.handle_event(window, event, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn handle_event(&mut self, window: &mut Window, event: UiEvent, cx: &mut Context<Self>) {
        match event {
            UiEvent::Probes(result) => {
                self.probes_loading = false;
                match result {
                    Ok(probes) => {
                        self.probe_message = if probes.is_empty() {
                            "未检测到 DAP-Link / CMSIS-DAP，请连接设备后刷新".to_string()
                        } else {
                            format!("检测到 {} 个 DAP-Link / CMSIS-DAP", probes.len())
                        };
                        self.logs
                            .push(format!("刷新调试器：{}", self.probe_message));

                        let previous = self
                            .selected_probe
                            .clone()
                            .or_else(|| self.config.last_probe.clone());
                        self.selected_probe = previous
                            .and_then(|key| {
                                probes
                                    .iter()
                                    .find(|probe| probe.key == key)
                                    .map(|probe| probe.key.clone())
                            })
                            .or_else(|| probes.first().map(|probe| probe.key.clone()));
                        self.probes = probes;
                    }
                    Err(error) => {
                        self.probes.clear();
                        self.selected_probe = None;
                        self.probe_message = error.clone();
                        self.logs.push(error);
                    }
                }
                self.persist_current_settings(cx);
                self.trim_logs();
                cx.notify();
            }
            UiEvent::Targets(targets) => {
                self.target_catalog_loading = false;
                self.target_catalog = targets;
                self.logs.push(format!(
                    "已加载 {} 个 probe-rs 目标芯片",
                    self.target_catalog.len()
                ));

                let items: Vec<SharedString> = self
                    .target_catalog
                    .iter()
                    .cloned()
                    .map(SharedString::from)
                    .collect();
                let wanted = self.config.last_target.clone();
                self.target_select.update(cx, |select, cx| {
                    select.set_items(SearchableVec::new(items), window, cx);
                    if !wanted.is_empty() {
                        let wanted = SharedString::from(wanted);
                        select.set_selected_value(&wanted, window, cx);
                    }
                });
                cx.notify();
            }
            UiEvent::Flash(event) => match event {
                FlashEvent::Log(message) => {
                    self.logs.push(message);
                    self.trim_logs();
                    cx.notify();
                }
                FlashEvent::Progress(progress) => {
                    self.progress_label = progress.operation.label().to_string();
                    self.progress_total = progress.total;
                    self.progress_percent = if progress.operation == FlashOperation::Fill {
                        0.0
                    } else {
                        progress.percent()
                    };
                    cx.notify();
                }
            },
            UiEvent::FlashFinished(result) => {
                self.busy = false;
                self.progress_total = None;
                match result {
                    Ok(outcome) => {
                        self.progress_percent = 100.0;
                        self.progress_label = "完成".to_string();
                        let verification = if outcome.dry_run {
                            "DRY RUN，无写入"
                        } else if outcome.verified {
                            "已写入并校验"
                        } else {
                            "已写入，未校验"
                        };
                        let message = format!(
                            "{} · {} · {} · {} ms",
                            outcome.target,
                            verification,
                            format_bytes(outcome.file_size),
                            outcome.elapsed.as_millis()
                        );
                        self.logs.push(format!("下载完成：{message}"));
                        self.result = Some(Ok(message));
                    }
                    Err(error) => {
                        self.progress_total = None;
                        self.logs.push(error.clone());
                        self.logs_open = true;
                        self.result = Some(Err(error));
                    }
                }
                self.trim_logs();
                cx.notify();
            }
        }
    }

    fn refresh_probes(&mut self, cx: &mut Context<Self>) {
        if self.probes_loading || self.busy {
            return;
        }
        self.probes_loading = true;
        self.probe_message = "正在扫描 USB 调试器……".to_string();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = backend::list_daplink_probes();
            let _ = sender.send_blocking(UiEvent::Probes(result));
        });
        cx.notify();
    }

    fn load_target_catalog(&mut self, cx: &mut Context<Self>) {
        if self.target_catalog_loading || !self.target_catalog.is_empty() {
            return;
        }
        self.target_catalog_loading = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let targets = backend::all_target_names();
            let _ = sender.send_blocking(UiEvent::Targets(targets));
        });
        cx.notify();
    }

    fn choose_probe(&mut self, key: ProbeKey, cx: &mut Context<Self>) {
        self.selected_probe = Some(key);
        self.persist_current_settings(cx);
        cx.notify();
    }

    fn choose_hex_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.choosing_file {
            return;
        }

        self.choosing_file = true;
        let mut dialog = FileDialog::new()
            .add_filter("Intel HEX 固件", &["hex"])
            .set_title("选择 Intel HEX 固件")
            .set_parent(window);
        if let Some(directory) = self.hex_path.as_ref().and_then(|path| path.parent())
            && directory.is_dir()
        {
            dialog = dialog.set_directory(directory);
        }

        let task = cx.background_spawn(async move { dialog.pick_file() });
        cx.spawn(async move |this, cx| {
            let selected = task.await;
            _ = this.update(cx, |this, cx| {
                this.choosing_file = false;
                if let Some(path) = selected {
                    this.set_hex_file(path, cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn set_hex_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match backend::validate_hex_path(&path) {
            Ok(size) => {
                self.hex_path = Some(path.clone());
                self.config.last_hex = Some(path.clone());
                self.result = None;
                self.logs.push(format!(
                    "已选择 HEX：{}（{}）",
                    path.display(),
                    format_bytes(size)
                ));
                self.trim_logs();
                if let Err(error) = config::save_config(&self.config) {
                    self.logs.push(error);
                }
            }
            Err(error) => {
                self.result = Some(Err(error.clone()));
                self.logs.push(error);
            }
        }
        cx.notify();
    }

    fn persist_current_settings(&mut self, cx: &mut Context<Self>) {
        let target = self.target_value(cx);
        if !target.is_empty() {
            self.config.last_target = target;
        }
        self.config.last_probe = self.selected_probe.clone();
        self.config.last_hex = self.hex_path.clone();
        self.config.verify = self.verify;
        if let Err(error) = config::save_config(&self.config) {
            self.logs.push(format!("保存配置失败：{error}"));
        }
    }

    fn target_value(&self, cx: &Context<Self>) -> String {
        self.target_select
            .read(cx)
            .selected_value()
            .map(|value| value.to_string())
            .unwrap_or_default()
    }

    fn hex_is_ready(&self) -> bool {
        self.hex_path
            .as_ref()
            .is_some_and(|path| path.is_file() && backend::validate_hex_path(path).is_ok())
    }

    fn can_download(&self, cx: &Context<Self>) -> bool {
        !self.busy
            && self.selected_probe.is_some()
            && !self.target_value(cx).is_empty()
            && self.hex_is_ready()
    }

    fn start_flash(&mut self, cx: &mut Context<Self>) {
        if !self.can_download(cx) {
            self.result = Some(Err(
                "请先选择调试器、有效的目标芯片和 HEX 文件。".to_string()
            ));
            cx.notify();
            return;
        }

        let Some(probe) = self.selected_probe.clone() else {
            return;
        };
        let Some(hex_path) = self.hex_path.clone() else {
            return;
        };

        let request = FlashRequest {
            probe,
            target: self.target_value(cx),
            hex_path,
            verify: self.verify,
        };

        self.busy = true;
        self.result = None;
        self.logs_open = false;
        self.progress_percent = 0.0;
        self.progress_total = None;
        self.progress_label = "连接目标芯片".to_string();
        self.logs.push(format!(
            "开始下载：{} → {}",
            request.hex_path.display(),
            request.target
        ));
        self.persist_current_settings(cx);

        let dry_run = std::env::var_os("ANYLINK_DRY_RUN").is_some();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let mut emit = |event| {
                let _ = sender.send_blocking(UiEvent::Flash(event));
            };
            let result = backend::flash_hex(&request, dry_run, &mut emit);
            let _ = sender.send_blocking(UiEvent::FlashFinished(result));
        });
        cx.notify();
    }

    fn trim_logs(&mut self) {
        const MAX_LOG_LINES: usize = 250;
        if self.logs.len() > MAX_LOG_LINES {
            let keep_from = self.logs.len() - MAX_LOG_LINES;
            self.logs.drain(..keep_from);
        }
    }

    fn render_probe_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let probes = self.probes.clone();
        let selected = self.selected_probe.clone();
        let busy = self.busy;
        let loading = self.probes_loading;

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child("调试器"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("仅列出 DAP-Link / CMSIS-DAP 设备"),
                            ),
                    )
                    .child(
                        Button::new("refresh-probes")
                            .small()
                            .label(if loading {
                                "扫描中…"
                            } else {
                                "刷新设备"
                            })
                            .loading(loading)
                            .disabled(busy || loading)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_probes(cx))),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.probe_message.clone()),
            )
            .when(!probes.is_empty(), |element| {
                element.child(
                    v_flex()
                        .gap_2()
                        .children(probes.into_iter().enumerate().map(|(index, probe)| {
                            let key = probe.key.clone();
                            let is_selected = selected.as_ref() == Some(&probe.key);
                            div()
                                .id(("probe", index))
                                .w_full()
                                .p_3()
                                .rounded_lg()
                                .border_2()
                                .border_color(if is_selected {
                                    cx.theme().primary
                                } else {
                                    cx.theme().border
                                })
                                .bg(cx.theme().secondary)
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.choose_probe(key.clone(), cx)
                                }))
                                .child(
                                    h_flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(div().text_sm().child(probe.name.clone()))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(probe.detail()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if is_selected {
                                                    cx.theme().green
                                                } else {
                                                    cx.theme().muted_foreground
                                                })
                                                .child(if is_selected {
                                                    "已选择"
                                                } else {
                                                    "点击选择"
                                                }),
                                        ),
                                )
                        })),
                )
            })
    }

    fn render_target_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let target_value = self.target_value(cx);
        let busy = self.busy;
        let loading = self.target_catalog_loading;

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("目标芯片"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if loading {
                                "正在加载芯片库…".to_string()
                            } else {
                                format!("{} 个型号", self.target_catalog.len())
                            }),
                    ),
            )
            .child(
                Select::new(&self.target_select)
                    .accessibility_label("目标芯片")
                    .placeholder("选择目标芯片")
                    .search_placeholder("输入型号快速过滤，例如 STM32H743ZI")
                    .highlight_matches(true)
                    .cleanable(true)
                    .disabled(busy)
                    .menu_max_h(px(340.))
                    .w_full(),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if target_value.is_empty() {
                        cx.theme().muted_foreground
                    } else {
                        cx.theme().green
                    })
                    .child(if target_value.is_empty() {
                        "点击下拉框选择；打开后可输入型号快速过滤".to_string()
                    } else {
                        format!("已选择：{target_value}")
                    }),
            )
    }

    fn render_file_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy;
        let choosing_file = self.choosing_file;
        let hex_path = self.hex_path.clone();
        let file_detail = match hex_path.as_ref() {
            Some(path) => match backend::validate_hex_path(path) {
                Ok(size) => format!("{} · {}", path.display(), format_bytes(size)),
                Err(error) => error,
            },
            None => "尚未选择固件".to_string(),
        };

        v_flex()
            .gap_2()
            .child(div().text_sm().child("Intel HEX 固件"))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .p_2()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .text_sm()
                            .child(file_detail),
                    )
                    .child(
                        Button::new("browse-hex")
                            .label(if choosing_file {
                                "选择中…"
                            } else {
                                "浏览…"
                            })
                            .loading(choosing_file)
                            .disabled(busy || choosing_file)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_hex_file(window, cx)),
                            ),
                    ),
            )
    }

    fn render_progress(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = self.busy && self.progress_total.is_none();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(if self.busy {
                        self.progress_label.clone()
                    } else {
                        "下载完成".to_string()
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if loading {
                                "进行中".to_string()
                            } else {
                                format!("{:.0}%", self.progress_percent)
                            }),
                    ),
            )
            .child(
                Progress::new("flash-progress")
                    .value(self.progress_percent)
                    .loading(loading),
            )
    }
}

impl Render for AnyLinkApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_download = self.can_download(cx);
        let busy = self.busy;
        let result = self.result.clone();
        let logs = self.logs.join("\n");
        let dry_run = std::env::var_os("ANYLINK_DRY_RUN").is_some();

        div()
            .id("app-page")
            .size_full()
            .overflow_y_scrollbar()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .p_6()
            .child(
                v_flex()
                    .max_w(px(880.0))
                    .mx_auto()
                    .gap_5()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        img("branding/logo.png")
                                            .id("anylink-logo")
                                            .size(px(62.0))
                                            .rounded(px(15.0))
                                            .flex_shrink_0(),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(div().text_2xl().child("AnyLink"))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("DAP-Link · STM32 · Intel HEX 固件下载"),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .when(dry_run, |element| {
                                        element.child(
                                            div()
                                                .px_3()
                                                .py_1()
                                                .rounded_full()
                                                .bg(cx.theme().secondary)
                                                .text_xs()
                                                .text_color(cx.theme().yellow)
                                                .child("DRY RUN"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1()
                                            .rounded_full()
                                            .bg(cx.theme().secondary)
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("v{APP_VERSION}")),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_5()
                            .p_5()
                            .rounded_xl()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .child(self.render_probe_section(cx))
                            .child(self.render_target_section(cx))
                            .child(self.render_file_section(cx)),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Checkbox::new("verify")
                                    .label("下载后回读校验")
                                    .checked(self.verify)
                                    .disabled(busy)
                                    .on_change(cx.listener(|this, checked, _, cx| {
                                        this.verify = *checked;
                                        this.persist_current_settings(cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("download")
                                    .primary()
                                    .large()
                                    .label(if busy {
                                        "正在下载……"
                                    } else {
                                        "一键下载"
                                    })
                                    .loading(busy)
                                    .disabled(!can_download)
                                    .on_click(cx.listener(|this, _, _, cx| this.start_flash(cx))),
                            ),
                    )
                    .when(busy || result.is_some(), |element| {
                        element.child(
                            v_flex()
                                .gap_3()
                                .p_4()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .child(self.render_progress(cx))
                                .when_some(result, |element, result| {
                                    element.child(match result {
                                        Ok(message) => div()
                                            .text_sm()
                                            .text_color(cx.theme().green)
                                            .child(format!("✓ {message}")),
                                        Err(error) => div()
                                            .text_sm()
                                            .text_color(cx.theme().red)
                                            .child(format!("✗ {error}")),
                                    })
                                }),
                        )
                    })
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("运行日志"),
                                    )
                                    .child(
                                        Button::new("toggle-logs")
                                            .ghost()
                                            .xsmall()
                                            .label(if self.logs_open { "收起" } else { "展开" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.logs_open = !this.logs_open;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .when(self.logs_open, |element| {
                                element.child(
                                    div()
                                        .id("log-view")
                                        .w_full()
                                        .max_h(px(190.0))
                                        .overflow_y_scroll()
                                        .p_3()
                                        .rounded_lg()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().secondary)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(logs),
                                )
                            }),
                    ),
            )
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
