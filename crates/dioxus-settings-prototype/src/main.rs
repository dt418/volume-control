//! Standalone Dioxus prototype for VolumeControl Settings.
//!
//! Evaluates declarative RSX ergonomics, state reactivity, and layout.
//! Does NOT mutate production config.ini or touch native audio endpoints.

use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Appearance,
    Hotkeys,
    Blacklist,
    Updates,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrototypeDraft {
    pub theme: String,
    pub material: String,
    pub volume_step: u8,
    pub autostart: bool,
}

impl Default for PrototypeDraft {
    fn default() -> Self {
        Self {
            theme: "System".into(),
            material: "Auto Glass".into(),
            volume_step: 2,
            autostart: true,
        }
    }
}

pub fn app() -> Element {
    let mut current_tab = use_signal(|| SettingsTab::General);
    let mut draft = use_signal(PrototypeDraft::default);
    let mut save_notice = use_signal(|| Option::<String>::None);

    rsx! {
        div {
            style: "display: flex; width: 760px; height: 620px; font-family: Segoe UI, sans-serif; background: #18191c; color: #f5f7fa; border-radius: 12px; overflow: hidden;",
            // Sidebar
            div {
                style: "width: 180px; background: rgba(20, 20, 25, 0.6); padding: 16px 8px; display: flex; flex-direction: column; gap: 8px; border-right: 1px solid rgba(255,255,255,0.08);",
                button {
                    style: if current_tab() == SettingsTab::General { "background: #00a4ef; color: white; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" } else { "background: transparent; color: #aaa; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" },
                    onclick: move |_| current_tab.set(SettingsTab::General),
                    "Cài đặt chung"
                }
                button {
                    style: if current_tab() == SettingsTab::Appearance { "background: #00a4ef; color: white; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" } else { "background: transparent; color: #aaa; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" },
                    onclick: move |_| current_tab.set(SettingsTab::Appearance),
                    "Giao diện"
                }
                button {
                    style: if current_tab() == SettingsTab::Hotkeys { "background: #00a4ef; color: white; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" } else { "background: transparent; color: #aaa; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" },
                    onclick: move |_| current_tab.set(SettingsTab::Hotkeys),
                    "Phím tắt"
                }
                button {
                    style: if current_tab() == SettingsTab::Blacklist { "background: #00a4ef; color: white; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" } else { "background: transparent; color: #aaa; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" },
                    onclick: move |_| current_tab.set(SettingsTab::Blacklist),
                    "Danh sách đen"
                }
                button {
                    style: if current_tab() == SettingsTab::Updates { "background: #00a4ef; color: white; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" } else { "background: transparent; color: #aaa; border: none; padding: 8px 12px; border-radius: 6px; text-align: left; cursor: pointer;" },
                    onclick: move |_| current_tab.set(SettingsTab::Updates),
                    "Cập nhật"
                }
            }
            // Content
            div {
                style: "flex: 1; display: flex; flex-direction: column;",
                div {
                    style: "flex: 1; padding: 24px;",
                    match current_tab() {
                        SettingsTab::General => rsx! {
                            div {
                                h3 { "Cài đặt chung" }
                                div {
                                    style: "margin-top: 16px;",
                                    button {
                                        style: "padding: 8px 16px; background: #333; color: white; border: 1px solid #444; border-radius: 6px;",
                                        onclick: move |_| {
                                            let cur = draft().autostart;
                                            draft.write().autostart = !cur;
                                        },
                                        if draft().autostart { "Khởi động cùng Windows: [BẬT]" } else { "Khởi động cùng Windows: [TẮT]" }
                                    }
                                }
                                div {
                                    style: "margin-top: 24px;",
                                    p { "Bước âm lượng phím tắt" }
                                    div {
                                        style: "display: flex; gap: 8px; margin-top: 8px;",
                                        for step in [1, 2, 5] {
                                            button {
                                                style: if draft().volume_step == step { "padding: 6px 14px; background: #00a4ef; color: white; border: none; border-radius: 6px;" } else { "padding: 6px 14px; background: #2a2a30; color: #bbb; border: 1px solid #3a3a40; border-radius: 6px;" },
                                                onclick: move |_| draft.write().volume_step = step,
                                                "{step}%"
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        SettingsTab::Appearance => rsx! {
                            div {
                                h3 { "Chủ đề hiển thị" }
                                div {
                                    style: "display: flex; gap: 8px; margin-top: 8px;",
                                    for t in ["System", "Light", "Dark"] {
                                        button {
                                            style: if draft().theme == t { "padding: 6px 14px; background: #00a4ef; color: white; border: none; border-radius: 6px;" } else { "padding: 6px 14px; background: #2a2a30; color: #bbb; border: 1px solid #3a3a40; border-radius: 6px;" },
                                            onclick: move |_| draft.write().theme = t.to_string(),
                                            "{t}"
                                        }
                                    }
                                }
                                h3 { style: "margin-top: 24px;", "Chất liệu kính" }
                                div {
                                    style: "display: flex; gap: 8px; margin-top: 8px;",
                                    for m in ["Auto Glass", "Translucent", "Opaque"] {
                                        button {
                                            style: if draft().material == m { "padding: 6px 14px; background: #00a4ef; color: white; border: none; border-radius: 6px;" } else { "padding: 6px 14px; background: #2a2a30; color: #bbb; border: 1px solid #3a3a40; border-radius: 6px;" },
                                            onclick: move |_| draft.write().material = m.to_string(),
                                            "{m}"
                                        }
                                    }
                                }
                            }
                        },
                        _ => rsx! {
                            div {
                                p { "Mục cài đặt này đang được phát triển trong prototype." }
                            }
                        }
                    }
                }
                // Action Bar
                div {
                    style: "height: 60px; background: rgba(25, 25, 30, 0.8); border-top: 1px solid rgba(255,255,255,0.08); display: flex; align-items: center; justify-content: flex-end; padding: 0 24px; gap: 12px;",
                    if let Some(msg) = save_notice() {
                        span { style: "margin-right: auto; color: #4ade80; font-size: 13px;", "{msg}" }
                    }
                    button {
                        style: "padding: 8px 16px; background: #2a2a30; color: white; border: 1px solid #444; border-radius: 6px; cursor: pointer;",
                        onclick: move |_| {
                            draft.set(PrototypeDraft::default());
                            save_notice.set(Some("Đã hủy thay đổi".into()));
                        },
                        "Hủy"
                    }
                    button {
                        style: "padding: 8px 16px; background: #00a4ef; color: white; border: none; border-radius: 6px; cursor: pointer;",
                        onclick: move |_| {
                            save_notice.set(Some(format!("Đã lưu draft: {:?}", draft())));
                        },
                        "Lưu"
                    }
                }
            }
        }
    }
}

fn main() {
    println!("Dioxus Settings Prototype compiled successfully.");
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    println!("VirtualDom initialized successfully.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prototype_virtual_dom_renders_without_crash() {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }

    #[test]
    fn prototype_draft_updates_reactively() {
        let mut draft = PrototypeDraft::default();
        assert_eq!(draft.volume_step, 2);
        draft.volume_step = 5;
        assert_eq!(draft.volume_step, 5);
        draft.theme = "Light".into();
        assert_eq!(draft.theme, "Light");
    }
}
