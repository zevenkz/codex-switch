//! 托盘菜单管理模块
//!
//! 负责系统托盘图标和菜单的创建、更新和事件处理。

use tauri::menu::{CheckMenuItem, Menu, MenuBuilder, MenuItem};
use tauri::{Emitter, Manager};

use crate::app_config::AppType;
use crate::codex_accounts::CodexAccountRecord;
use crate::error::AppError;
use crate::store::AppState;

/// 托盘菜单文本（国际化）
#[derive(Clone, Copy)]
pub struct TrayTexts {
    pub show_main: &'static str,
    pub refresh_quota: &'static str,
    pub no_accounts_label: &'static str,
    pub quit: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexTrayAccountEntry {
    id: String,
    label: String,
    enabled: bool,
    checked: bool,
}

impl TrayTexts {
    pub fn from_language(language: &str) -> Self {
        match language {
            "en" => Self {
                show_main: "Open main window",
                refresh_quota: "Refresh quota",
                no_accounts_label: "(no accounts)",
                quit: "Quit",
            },
            "ja" => Self {
                show_main: "メインウィンドウを開く",
                refresh_quota: "利用枠を更新",
                no_accounts_label: "(アカウントなし)",
                quit: "終了",
            },
            _ => Self {
                show_main: "打开主界面",
                refresh_quota: "刷新额度",
                no_accounts_label: "(无账号)",
                quit: "退出",
            },
        }
    }
}

fn codex_tray_account_entries(accounts: &[CodexAccountRecord]) -> Vec<CodexTrayAccountEntry> {
    accounts
        .iter()
        .map(|account| CodexTrayAccountEntry {
            id: format!("codex_account::{}", account.id),
            label: account
                .email
                .clone()
                .or(account.display_name.clone())
                .unwrap_or_else(|| account.id.clone()),
            enabled: !account.is_active,
            checked: account.is_active,
        })
        .collect()
}

/// 托盘应用分区配置
pub struct TrayAppSection {
    pub app_type: AppType,
    pub prefix: &'static str,
    pub log_name: &'static str,
}

/// Auto 菜单项后缀
pub const AUTO_SUFFIX: &str = "auto";

pub const TRAY_SECTIONS: [TrayAppSection; 3] = [
    TrayAppSection {
        app_type: AppType::Claude,
        prefix: "claude_",
        log_name: "Claude",
    },
    TrayAppSection {
        app_type: AppType::Codex,
        prefix: "codex_",
        log_name: "Codex",
    },
    TrayAppSection {
        app_type: AppType::Gemini,
        prefix: "gemini_",
        log_name: "Gemini",
    },
];

/// 处理供应商托盘事件
pub fn handle_provider_tray_event(app: &tauri::AppHandle, event_id: &str) -> bool {
    for section in TRAY_SECTIONS.iter() {
        if let Some(suffix) = event_id.strip_prefix(section.prefix) {
            // 处理 Auto 点击
            if suffix == AUTO_SUFFIX {
                log::info!("切换到{} Auto模式", section.log_name);
                let app_handle = app.clone();
                let app_type = section.app_type.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(e) = handle_auto_click(&app_handle, &app_type) {
                        log::error!("切换{}Auto模式失败: {e}", section.log_name);
                    }
                });
                return true;
            }

            // 处理供应商点击
            log::info!("切换到{}供应商: {suffix}", section.log_name);
            let app_handle = app.clone();
            let provider_id = suffix.to_string();
            let app_type = section.app_type.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = handle_provider_click(&app_handle, &app_type, &provider_id) {
                    log::error!("切换{}供应商失败: {e}", section.log_name);
                }
            });
            return true;
        }
    }
    false
}

/// 处理 Auto 点击：启用 proxy 和 auto_failover
fn handle_auto_click(app: &tauri::AppHandle, app_type: &AppType) -> Result<(), AppError> {
    if let Some(app_state) = app.try_state::<AppState>() {
        let app_type_str = app_type.as_str();

        // 强一致语义：Auto 模式开启后立即切到队列 P1（P1→P2→...）
        // 若队列为空，则尝试把“当前供应商”自动加入队列作为 P1，避免用户陷入无法开启的死锁。
        let mut queue = app_state.db.get_failover_queue(app_type_str)?;
        if queue.is_empty() {
            let current_id =
                crate::settings::get_effective_current_provider(&app_state.db, app_type)?;
            let Some(current_id) = current_id else {
                return Err(AppError::Message(
                    "故障转移队列为空，且未设置当前供应商，无法启用 Auto 模式".to_string(),
                ));
            };
            app_state
                .db
                .add_to_failover_queue(app_type_str, &current_id)?;
            queue = app_state.db.get_failover_queue(app_type_str)?;
        }

        let p1_provider_id = queue
            .first()
            .map(|item| item.provider_id.clone())
            .ok_or_else(|| AppError::Message("故障转移队列为空，无法启用 Auto 模式".to_string()))?;

        // 真正启用 failover：启动代理服务 + 执行接管 + 开启 auto_failover
        let proxy_service = &app_state.proxy_service;

        // 1) 确保代理服务运行（会自动设置 proxy_enabled = true）
        let is_running = futures::executor::block_on(proxy_service.is_running());
        if !is_running {
            log::info!("[Tray] Auto 模式：启动代理服务");
            if let Err(e) = futures::executor::block_on(proxy_service.start()) {
                log::error!("[Tray] 启动代理服务失败: {e}");
                return Err(AppError::Message(format!("启动代理服务失败: {e}")));
            }
        }

        // 2) 执行 Live 配置接管（确保该 app 被代理接管）
        log::info!("[Tray] Auto 模式：对 {app_type_str} 执行接管");
        if let Err(e) =
            futures::executor::block_on(proxy_service.set_takeover_for_app(app_type_str, true))
        {
            log::error!("[Tray] 执行接管失败: {e}");
            return Err(AppError::Message(format!("执行接管失败: {e}")));
        }

        // 3) 设置 auto_failover_enabled = true
        app_state
            .db
            .set_proxy_flags_sync(app_type_str, true, true)?;

        // 3.1) 立即切到队列 P1（热切换：不写 Live，仅更新 DB/settings/备份）
        if let Err(e) = futures::executor::block_on(
            proxy_service.switch_proxy_target(app_type_str, &p1_provider_id),
        ) {
            log::error!("[Tray] Auto 模式切换到队列 P1 失败: {e}");
            return Err(AppError::Message(format!(
                "Auto 模式切换到队列 P1 失败: {e}"
            )));
        }

        // 4) 更新托盘菜单
        if let Ok(new_menu) = create_tray_menu(app, app_state.inner()) {
            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_menu(Some(new_menu));
            }
        }

        // 5) 发射事件到前端
        let event_data = serde_json::json!({
            "appType": app_type_str,
            "proxyEnabled": true,
            "autoFailoverEnabled": true,
            "providerId": p1_provider_id
        });
        if let Err(e) = app.emit("proxy-flags-changed", event_data.clone()) {
            log::error!("发射 proxy-flags-changed 事件失败: {e}");
        }
        // 发射 provider-switched 事件（保持向后兼容，Auto 切换也算一种切换）
        if let Err(e) = app.emit("provider-switched", event_data) {
            log::error!("发射 provider-switched 事件失败: {e}");
        }
    }
    Ok(())
}

/// 处理供应商点击：关闭 auto_failover + 切换供应商
fn handle_provider_click(
    app: &tauri::AppHandle,
    app_type: &AppType,
    provider_id: &str,
) -> Result<(), AppError> {
    if let Some(app_state) = app.try_state::<AppState>() {
        let app_type_str = app_type.as_str();

        // 获取当前 proxy 状态，保持 enabled 不变，只关闭 auto_failover
        let (proxy_enabled, _) = app_state.db.get_proxy_flags_sync(app_type_str);
        app_state
            .db
            .set_proxy_flags_sync(app_type_str, proxy_enabled, false)?;

        // 切换供应商
        crate::commands::switch_provider(
            app_state.clone(),
            app_type_str.to_string(),
            provider_id.to_string(),
        )
        .map_err(AppError::Message)?;

        // 更新托盘菜单
        if let Ok(new_menu) = create_tray_menu(app, app_state.inner()) {
            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_menu(Some(new_menu));
            }
        }

        // 发射事件到前端
        let event_data = serde_json::json!({
            "appType": app_type_str,
            "proxyEnabled": proxy_enabled,
            "autoFailoverEnabled": false,
            "providerId": provider_id
        });
        if let Err(e) = app.emit("proxy-flags-changed", event_data.clone()) {
            log::error!("发射 proxy-flags-changed 事件失败: {e}");
        }
        // 发射 provider-switched 事件（保持向后兼容）
        if let Err(e) = app.emit("provider-switched", event_data) {
            log::error!("发射 provider-switched 事件失败: {e}");
        }
    }
    Ok(())
}

/// 创建动态托盘菜单
pub fn create_tray_menu(
    app: &tauri::AppHandle,
    app_state: &AppState,
) -> Result<Menu<tauri::Wry>, AppError> {
    let app_settings = crate::settings::get_settings();
    let tray_texts = TrayTexts::from_language(app_settings.language.as_deref().unwrap_or("zh"));
    let mut menu_builder = MenuBuilder::new(app);

    let show_main_item =
        MenuItem::with_id(app, "show_main", tray_texts.show_main, true, None::<&str>)
            .map_err(|e| AppError::Message(format!("创建打开主界面菜单失败: {e}")))?;
    let refresh_item =
        MenuItem::with_id(app, "refresh_quota", tray_texts.refresh_quota, true, None::<&str>)
            .map_err(|e| AppError::Message(format!("创建刷新额度菜单失败: {e}")))?;

    menu_builder = menu_builder
        .item(&show_main_item)
        .item(&refresh_item)
        .separator();

    let accounts = app_state
        .codex_accounts
        .lock()
        .map_err(|e| AppError::Message(format!("读取 Codex 账号菜单失败: {e}")))?
        .store
        .list_accounts()?;

    if accounts.is_empty() {
        let empty_item =
            MenuItem::with_id(app, "codex_account_empty", tray_texts.no_accounts_label, false, None::<&str>)
                .map_err(|e| AppError::Message(format!("创建空账号菜单失败: {e}")))?;
        menu_builder = menu_builder.item(&empty_item);
    } else {
        for entry in codex_tray_account_entries(&accounts) {
            let item = CheckMenuItem::with_id(
                app,
                &entry.id,
                &entry.label,
                entry.enabled,
                entry.checked,
                None::<&str>,
            )
            .map_err(|e| AppError::Message(format!("创建 Codex 账号菜单失败: {e}")))?;
            menu_builder = menu_builder.item(&item);
        }
    }

    menu_builder = menu_builder.separator();

    let quit_item = MenuItem::with_id(app, "quit", tray_texts.quit, true, None::<&str>)
        .map_err(|e| AppError::Message(format!("创建退出菜单失败: {e}")))?;

    menu_builder = menu_builder.item(&quit_item);

    menu_builder
        .build()
        .map_err(|e| AppError::Message(format!("构建菜单失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::codex_tray_account_entries;
    use crate::codex_accounts::CodexAccountRecord;

    fn sample_account(
        id: &str,
        email: Option<&str>,
        display_name: Option<&str>,
        is_active: bool,
    ) -> CodexAccountRecord {
        CodexAccountRecord {
            id: id.to_string(),
            email: email.map(str::to_string),
            account_id: Some(id.to_string()),
            plan_type: Some("plus".to_string()),
            display_name: display_name.map(str::to_string),
            avatar_seed: "C".to_string(),
            added_at: 1,
            last_used_at: None,
            is_active,
            auth_json: serde_json::json!({}),
            quota: None,
            metadata: serde_json::Map::new(),
        }
    }

    #[test]
    fn codex_tray_account_entries_marks_active_account_checked_and_disabled() {
        let entries = codex_tray_account_entries(&[
            sample_account("acct-active", Some("active@example.com"), Some("Active"), true),
            sample_account("acct-other", Some("other@example.com"), Some("Other"), false),
        ]);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "codex_account::acct-active");
        assert_eq!(entries[0].label, "active@example.com");
        assert!(!entries[0].enabled);
        assert!(entries[0].checked);

        assert_eq!(entries[1].id, "codex_account::acct-other");
        assert_eq!(entries[1].label, "other@example.com");
        assert!(entries[1].enabled);
        assert!(!entries[1].checked);
    }

    #[test]
    fn codex_tray_account_entries_falls_back_to_display_name_then_id() {
        let entries = codex_tray_account_entries(&[
            sample_account("acct-display", None, Some("Display Only"), false),
            sample_account("acct-id", None, None, false),
        ]);

        assert_eq!(entries[0].label, "Display Only");
        assert_eq!(entries[1].label, "acct-id");
    }
}

pub fn refresh_tray_menu(app: &tauri::AppHandle) {
    use crate::store::AppState;

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(new_menu) = create_tray_menu(app, state.inner()) {
            if let Some(tray) = app.tray_by_id("main") {
                if let Err(e) = tray.set_menu(Some(new_menu)) {
                    log::error!("刷新托盘菜单失败: {e}");
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn apply_tray_policy(app: &tauri::AppHandle, dock_visible: bool) {
    use tauri::ActivationPolicy;

    let desired_policy = if dock_visible {
        ActivationPolicy::Regular
    } else {
        ActivationPolicy::Accessory
    };

    if let Err(err) = app.set_dock_visibility(dock_visible) {
        log::warn!("设置 Dock 显示状态失败: {err}");
    }

    if let Err(err) = app.set_activation_policy(desired_policy) {
        log::warn!("设置激活策略失败: {err}");
    }
}

/// 处理托盘菜单事件
pub fn handle_tray_menu_event(app: &tauri::AppHandle, event_id: &str) {
    log::info!("处理托盘菜单事件: {event_id}");

    match event_id {
        "show_main" => {
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                {
                    let _ = window.set_skip_taskbar(false);
                }
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                #[cfg(target_os = "macos")]
                {
                    apply_tray_policy(app, true);
                }
            } else if crate::lightweight::is_lightweight_mode() {
                if let Err(e) = crate::lightweight::exit_lightweight_mode(app) {
                    log::error!("退出轻量模式重建窗口失败: {e}");
                }
            }
        }
        "refresh_quota" => {
            if app.try_state::<AppState>().is_some() {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let Some(app_state) = app_handle.try_state::<AppState>() else {
                        return;
                    };
                    let result = crate::commands::refresh_all_codex_account_quotas_internal(
                        app_state.inner(),
                    )
                    .await;

                    refresh_tray_menu(&app_handle);

                    match result {
                        Ok(_) => {
                            let _ = app_handle.emit(
                                "codex-accounts-updated",
                                serde_json::json!({ "source": "tray" }),
                            );
                        }
                        Err(error) => {
                            log::error!("刷新 Codex 额度失败: {error}");
                            let _ = app_handle.emit(
                                "codex-accounts-updated",
                                serde_json::json!({
                                    "source": "tray",
                                    "error": error.to_string(),
                                }),
                            );
                        }
                    }
                });
            }
        }
        "quit" => {
            log::info!("退出应用");
            app.exit(0);
        }
        _ => {
            if let Some(account_id) = event_id.strip_prefix("codex_account::") {
                if app.try_state::<AppState>().is_some() {
                    let app_handle = app.clone();
                    let account_id = account_id.to_string();
                    tauri::async_runtime::spawn_blocking(move || {
                        let Some(app_state) = app_handle.try_state::<AppState>() else {
                            return;
                        };
                        match crate::commands::switch_codex_account_and_restart_internal(app_state.inner(), &account_id) {
                            Ok(_) => {
                                refresh_tray_menu(&app_handle);
                                let _ = app_handle.emit("codex-accounts-updated", ());
                            }
                            Err(error) => {
                                log::error!("托盘切换 Codex 账号失败: {error}");
                            }
                        }
                    });
                }
                return;
            }
            if handle_provider_tray_event(app, event_id) {
                return;
            }
            log::warn!("未处理的菜单事件: {event_id}");
        }
    }
}
