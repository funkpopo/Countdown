use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::localization::{resolve_ui_language, UiLanguage};
use crate::{background_sync, compat_api};

const TRAY_ID: &str = "main-tray";
const QUICK_VIEW_LABEL: &str = "quick-view";
const OPEN_MAIN_LABEL: &str = "open-main";
const OPEN_TODAY_REPORT_LABEL: &str = "open-today-report";
const TOGGLE_COMPAT_API_LABEL: &str = "toggle-compat-api";
const SYNC_NOW_LABEL: &str = "sync-now";
const QUIT_LABEL: &str = "quit";
const MAIN_WINDOW_NAVIGATE_EVENT: &str = "main-window-navigate";
const COMPAT_API_STATUS_CHANGED_EVENT: &str = "compat-api-status-changed";

const HOVER_CARD_WIDTH: f64 = 320.0;
const HOVER_CARD_HEIGHT: f64 = 344.0;
const HOVER_DELAY_MS: u64 = 1000;
const HOVER_HIDE_GRACE_MS: u64 = 650;

#[derive(Clone)]
pub struct TrayRuntime {
    quick_view_visible: Arc<AtomicBool>,
    hover_request_id: Arc<AtomicU64>,
}

impl TrayRuntime {
    pub fn new() -> Self {
        Self {
            quick_view_visible: Arc::new(AtomicBool::new(false)),
            hover_request_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn setup(&self, app: &AppHandle) -> Result<(), String> {
        app.manage(self.clone());

        let tray_icon = app
            .default_window_icon()
            .cloned()
            .ok_or_else(|| "missing default window icon".to_string())?;

        let quick_view_visible = self.quick_view_visible.clone();
        let hover_request_id = self.hover_request_id.clone();
        let menu_quick_view_visible = self.quick_view_visible.clone();
        let menu_hover_request_id = self.hover_request_id.clone();

        let tray = TrayIconBuilder::with_id(TRAY_ID)
            .icon(tray_icon)
            .show_menu_on_left_click(false)
            .on_tray_icon_event(move |tray, event| {
                let app_handle = tray.app_handle();
                match event {
                    tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } => {
                        handle_left_click(&app_handle, &quick_view_visible, &hover_request_id);
                    }
                    tauri::tray::TrayIconEvent::Enter { rect, .. } => {
                        schedule_hover_preview(
                            &app_handle,
                            &rect,
                            &quick_view_visible,
                            &hover_request_id,
                        );
                    }
                    tauri::tray::TrayIconEvent::Leave { .. } => {
                        schedule_hover_dismiss(&app_handle, &quick_view_visible, &hover_request_id);
                    }
                    _ => {}
                }
            })
            .build(app)
            .map_err(|e| e.to_string())?;

        let menu = build_tray_menu(app, resolve_ui_language(app), false)?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;

        tray.on_menu_event(move |app, event| match event.id.as_ref() {
            OPEN_MAIN_LABEL => {
                let _ =
                    dismiss_hover_preview(&app, &menu_quick_view_visible, &menu_hover_request_id);
                let _ = show_main_window(&app);
            }
            OPEN_TODAY_REPORT_LABEL => {
                let _ =
                    dismiss_hover_preview(&app, &menu_quick_view_visible, &menu_hover_request_id);
                let _ = show_main_window_page(&app, "overview", Some("today"));
            }
            TOGGLE_COMPAT_API_LABEL => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    match compat_api::toggle_server(&app_handle).await {
                        Ok(status) => {
                            let _ = app_handle.emit(COMPAT_API_STATUS_CHANGED_EVENT, status);
                        }
                        Err(error) => {
                            eprintln!("Failed to toggle Compat API server from tray: {error}");
                        }
                    }
                    let _ = refresh_tray_menu(&app_handle).await;
                });
            }
            SYNC_NOW_LABEL => {
                background_sync::sync_now(app.clone());
            }
            QUIT_LABEL => {
                app.exit(0);
            }
            _ => {}
        });

        Ok(())
    }

    pub fn handle_window_event(&self, window: &tauri::Window, event: &tauri::WindowEvent) {
        if let tauri::WindowEvent::Focused(true) = event {
            if window.label() != QUICK_VIEW_LABEL {
                let _ = self.cancel_hover_for_app(window.app_handle());
            }
        }
    }

    pub fn cancel_hover_for_app(&self, app: &AppHandle) -> Result<(), String> {
        dismiss_hover_preview(app, &self.quick_view_visible, &self.hover_request_id)
    }

    pub fn cancel_pending_hover_action(&self) {
        self.hover_request_id.fetch_add(1, Ordering::SeqCst);
    }
}

fn handle_left_click(
    app: &AppHandle,
    quick_view_visible: &Arc<AtomicBool>,
    hover_request_id: &Arc<AtomicU64>,
) {
    let _ = dismiss_hover_preview(app, quick_view_visible, hover_request_id);
    let _ = show_main_window(app);
}

fn schedule_hover_preview(
    app: &AppHandle,
    tray_rect: &tauri::Rect,
    quick_view_visible: &Arc<AtomicBool>,
    hover_request_id: &Arc<AtomicU64>,
) {
    let request_id = hover_request_id.fetch_add(1, Ordering::SeqCst) + 1;
    let app_handle = app.clone();
    let rect = *tray_rect;
    let visible = quick_view_visible.clone();
    let request_tracker = hover_request_id.clone();

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(HOVER_DELAY_MS));
        if request_tracker.load(Ordering::SeqCst) != request_id {
            return;
        }

        let _ = show_quick_view_window(&app_handle, &rect, &visible);
    });
}

fn schedule_hover_dismiss(
    app: &AppHandle,
    quick_view_visible: &Arc<AtomicBool>,
    hover_request_id: &Arc<AtomicU64>,
) {
    let request_id = hover_request_id.fetch_add(1, Ordering::SeqCst) + 1;
    if !quick_view_visible.load(Ordering::SeqCst) {
        return;
    }

    let app_handle = app.clone();
    let visible = quick_view_visible.clone();
    let request_tracker = hover_request_id.clone();

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(HOVER_HIDE_GRACE_MS));
        if request_tracker.load(Ordering::SeqCst) != request_id {
            return;
        }

        let _ = hide_quick_view(&app_handle, &visible);
    });
}

fn dismiss_hover_preview(
    app: &AppHandle,
    quick_view_visible: &Arc<AtomicBool>,
    hover_request_id: &Arc<AtomicU64>,
) -> Result<(), String> {
    hover_request_id.fetch_add(1, Ordering::SeqCst);
    if quick_view_visible.load(Ordering::SeqCst) {
        hide_quick_view(app, quick_view_visible)?;
    }
    Ok(())
}

pub async fn refresh_tray_menu(app: &AppHandle) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };

    let language = resolve_ui_language(app);
    let compat_status = compat_api::get_status(app).await;
    let menu = build_tray_menu(app, language, compat_status.running)?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())
}

fn build_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    language: UiLanguage,
    compat_api_running: bool,
) -> Result<Menu<R>, String> {
    let open_main =
        MenuItemBuilder::with_id(OPEN_MAIN_LABEL, tray_text(language, TrayText::OpenMain))
            .build(app)
            .map_err(|e| e.to_string())?;
    let open_today_report = MenuItemBuilder::with_id(
        OPEN_TODAY_REPORT_LABEL,
        tray_text(language, TrayText::OpenTodayReport),
    )
    .build(app)
    .map_err(|e| e.to_string())?;
    let toggle_compat_api = MenuItemBuilder::with_id(
        TOGGLE_COMPAT_API_LABEL,
        tray_text(
            language,
            if compat_api_running {
                TrayText::StopCompatApi
            } else {
                TrayText::StartCompatApi
            },
        ),
    )
    .build(app)
    .map_err(|e| e.to_string())?;
    let sync_now = MenuItemBuilder::with_id(SYNC_NOW_LABEL, tray_text(language, TrayText::SyncNow))
        .build(app)
        .map_err(|e| e.to_string())?;
    let separator_top = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let separator_bottom = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit = PredefinedMenuItem::quit(app, Some(tray_text(language, TrayText::Quit)))
        .map_err(|e| e.to_string())?;

    MenuBuilder::new(app)
        .item(&open_main)
        .item(&open_today_report)
        .item(&separator_top)
        .item(&toggle_compat_api)
        .item(&sync_now)
        .item(&separator_bottom)
        .item(&quit)
        .build()
        .map_err(|e| e.to_string())
}

enum TrayText {
    OpenMain,
    OpenTodayReport,
    StartCompatApi,
    StopCompatApi,
    SyncNow,
    Quit,
}

fn tray_text(language: UiLanguage, text: TrayText) -> &'static str {
    match (language, text) {
        (UiLanguage::En, TrayText::OpenMain) => "Open Main Window",
        (UiLanguage::En, TrayText::OpenTodayReport) => "Open Today's Report",
        (UiLanguage::En, TrayText::StartCompatApi) => "Start Compat API",
        (UiLanguage::En, TrayText::StopCompatApi) => "Stop Compat API",
        (UiLanguage::En, TrayText::SyncNow) => "Sync Now",
        (UiLanguage::En, TrayText::Quit) => "Quit",
        (UiLanguage::Zh, TrayText::OpenMain) => "打开主窗口",
        (UiLanguage::Zh, TrayText::OpenTodayReport) => "打开今日报表",
        (UiLanguage::Zh, TrayText::StartCompatApi) => "启动兼容 API",
        (UiLanguage::Zh, TrayText::StopCompatApi) => "停止兼容 API",
        (UiLanguage::Zh, TrayText::SyncNow) => "立即同步",
        (UiLanguage::Zh, TrayText::Quit) => "退出",
    }
}

fn show_quick_view_window(
    app: &AppHandle,
    tray_rect: &tauri::Rect,
    quick_view_visible: &Arc<AtomicBool>,
) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window(QUICK_VIEW_LABEL) {
        window
    } else {
        WebviewWindowBuilder::new(app, QUICK_VIEW_LABEL, WebviewUrl::App("quick-view".into()))
            .title("Countdown - Hover Card")
            .inner_size(HOVER_CARD_WIDTH, HOVER_CARD_HEIGHT)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .decorations(false)
            .transparent(true)
            .visible(false)
            .focused(false)
            .build()
            .map_err(|e: tauri::Error| e.to_string())?
    };

    let position = calculate_window_position(tray_rect);
    window
        .set_position(PhysicalPosition::new(position.0, position.1))
        .map_err(|e| e.to_string())?;

    window
        .set_size(PhysicalSize::new(HOVER_CARD_WIDTH, HOVER_CARD_HEIGHT))
        .map_err(|e| e.to_string())?;

    window.show().map_err(|e| e.to_string())?;
    quick_view_visible.store(true, Ordering::SeqCst);

    Ok(())
}

fn hide_quick_view(app: &AppHandle, quick_view_visible: &Arc<AtomicBool>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(QUICK_VIEW_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    quick_view_visible.store(false, Ordering::SeqCst);
    Ok(())
}

fn calculate_window_position(tray_rect: &tauri::Rect) -> (i32, i32) {
    let (x, y) = match tray_rect.position {
        tauri::Position::Physical(pos) => (pos.x, pos.y),
        tauri::Position::Logical(pos) => (pos.x as i32, pos.y as i32),
    };

    let window_y = if y > HOVER_CARD_HEIGHT as i32 {
        y - HOVER_CARD_HEIGHT as i32 - 12
    } else {
        y + 36
    };

    let window_x = x - (HOVER_CARD_WIDTH as i32 / 2);

    (window_x.max(0), window_y.max(0))
}

pub(crate) fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        if window.is_minimized().map_err(|e| e.to_string())? {
            window.unminimize().map_err(|e| e.to_string())?;
        }
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    Ok(())
}

pub(crate) fn show_main_window_page(
    app: &AppHandle,
    page: &str,
    period: Option<&str>,
) -> Result<(), String> {
    show_main_window(app)?;
    app.emit_to(
        "main",
        MAIN_WINDOW_NAVIGATE_EVENT,
        json!({
            "page": page,
            "period": period,
        }),
    )
    .map_err(|e| e.to_string())
}
