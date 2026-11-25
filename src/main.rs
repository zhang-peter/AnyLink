#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

use std::borrow::Cow;

use anylink::{APP_NAME, APP_VERSION};
use app::AnyLinkApp;
use gpui_kit::assets::Assets as GpuiAssets;
use gpui_kit::component::{Theme, scroll::ScrollbarMode};
use gpui_kit::*;

#[derive(Clone, Copy)]
struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "branding/logo.png" {
            return Ok(Some(Cow::Borrowed(
                include_bytes!("../assets/logo.png").as_slice(),
            )));
        }
        GpuiAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut items = GpuiAssets.list(path)?;
        let logo = "branding/logo.png";
        if logo.starts_with(path) {
            items.push(logo.into());
        }
        Ok(items)
    }
}

fn main() {
    application().with_assets(AppAssets).run(|cx| {
        init(cx);
        Theme::set_scrollbar_mode(ScrollbarMode::Always, cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(
                None,
                size(px(1040.0), px(820.0)),
                cx,
            )),
            titlebar: Some(TitlebarOptions {
                title: Some(format!("{APP_NAME} v{APP_VERSION}").into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(860.0), px(620.0))),
            ..Default::default()
        };

        open_window(window_options, cx, |window, cx| {
            let view = cx.new(|cx| AnyLinkApp::new(window, cx));
            view.update(cx, |this, cx| this.start(window, cx));
            view
        })
        .expect("Failed to open AnyLink window");
    });
}
