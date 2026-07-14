//! The native settings + control window (egui / eframe).
//!
//! Shares a [`Runtime`] with the background API server: it reads live status
//! from the supervisor, drives start/stop/restart, and edits settings that are
//! persisted to `bridge.toml` and applied live.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;

use crate::config::{self, Config, ServerProcessConfig};
use crate::runtime::Runtime;
use crate::supervisor::{ServerStatus, Supervisor, SupervisorError};

const GREEN: egui::Color32 = egui::Color32::from_rgb(0x3a, 0xd1, 0x9a);
const AMBER: egui::Color32 = egui::Color32::from_rgb(0xe0, 0x7a, 0x5a);

pub struct BridgeApp {
    runtime: Arc<Runtime>,
    bind: String,
    port: String,
    token: String,
    save_dir: String,
    exe: String,
    args: String,
    allow_writes: bool,
    message: String,
}

impl BridgeApp {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let c = runtime.snapshot();
        let (exe, args) = match &c.server_process {
            Some(sp) => (sp.exe.to_string_lossy().to_string(), sp.args.join("\n")),
            None => (String::new(), String::new()),
        };
        Self {
            bind: c.bind,
            port: c.port.to_string(),
            token: c.token,
            save_dir: c.save_dir.to_string_lossy().to_string(),
            exe,
            args,
            allow_writes: c.allow_writes,
            message: String::new(),
            runtime,
        }
    }

    /// Validate the form and build a `Config` from it.
    fn build_config(&self) -> Result<Config, String> {
        let port: u16 = self
            .port
            .trim()
            .parse()
            .map_err(|_| "Port must be a whole number between 1 and 65535.".to_string())?;
        if port == 0 {
            return Err("Port must be between 1 and 65535.".into());
        }
        let bind = self.bind.trim();
        if bind.is_empty() {
            return Err("Bind IP can't be empty.".into());
        }
        let token = self.token.trim();
        if token.len() < 16 {
            return Err("Token should be at least 16 characters (use Generate).".into());
        }
        let save_dir = self.save_dir.trim().replace('\\', "/");
        let exe = self.exe.trim().replace('\\', "/");
        let server_process = if exe.is_empty() {
            None
        } else {
            let args = self
                .args
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            Some(ServerProcessConfig {
                exe: PathBuf::from(exe),
                args,
            })
        };
        Ok(Config {
            bind: bind.to_string(),
            port,
            token: token.to_string(),
            save_dir: PathBuf::from(save_dir),
            allow_writes: self.allow_writes,
            server_process,
        })
    }

    /// Run a supervisor operation off the UI thread so the window never freezes.
    fn spawn_op<F>(&self, label: &'static str, op: F)
    where
        F: FnOnce(&Supervisor) -> Result<ServerStatus, SupervisorError> + Send + 'static,
    {
        let sup = self.runtime.supervisor.clone();
        let rt = self.runtime.clone();
        std::thread::spawn(move || match op(&sup) {
            Ok(_) => rt.log(format!("server {label}: ok")),
            Err(e) => rt.log(format!("server {label} failed: {e}")),
        });
    }

    fn on_save(&mut self) {
        match self.build_config() {
            Ok(cfg) => match self.runtime.apply(cfg) {
                Ok(()) => {
                    self.message = "Saved — applied live.".into();
                    self.runtime.log("settings saved");
                }
                Err(e) => self.message = format!("Save failed: {e}"),
            },
            Err(e) => self.message = e,
        }
    }
}

impl eframe::App for BridgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keep the status readout fresh even without user interaction.
        ctx.request_repaint_after(Duration::from_millis(1000));
        let status = self.runtime.supervisor.status();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PSM Bridge");
            ui.label(self.runtime.bind_status());
            ui.separator();

            // --- Game server ---
            ui.horizontal(|ui| {
                let (color, text) = if status.running {
                    (
                        GREEN,
                        format!(
                            "Server running — PID {}, up {}s",
                            status.pid.unwrap_or(0),
                            status.uptime_secs.unwrap_or(0)
                        ),
                    )
                } else if status.configured {
                    (AMBER, "Server stopped".to_string())
                } else {
                    (
                        egui::Color32::GRAY,
                        "Server not set up — set the executable below".to_string(),
                    )
                };
                ui.colored_label(color, "●");
                ui.label(text);
            });
            ui.horizontal(|ui| {
                if status.running {
                    if ui.button("Restart").clicked() {
                        self.spawn_op("restart", Supervisor::restart);
                    }
                    if ui.button("Stop").clicked() {
                        self.spawn_op("stop", Supervisor::stop);
                    }
                } else if ui
                    .add_enabled(status.configured, egui::Button::new("Start"))
                    .clicked()
                {
                    self.spawn_op("start", Supervisor::start);
                }
            });

            ui.separator();
            ui.heading("Settings");

            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Bind IP");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.bind).desired_width(140.0));
                        if ui.small_button("localhost").clicked() {
                            self.bind = "127.0.0.1".into();
                        }
                        if ui.small_button("LAN (0.0.0.0)").clicked() {
                            self.bind = "0.0.0.0".into();
                        }
                    });
                    ui.end_row();

                    ui.label("Port");
                    ui.add(egui::TextEdit::singleline(&mut self.port).desired_width(100.0));
                    ui.end_row();

                    ui.label("Token");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.token).desired_width(240.0));
                        if ui.small_button("Generate").clicked() {
                            self.token = config::new_token();
                        }
                        if ui.small_button("Copy").clicked() {
                            ctx.copy_text(self.token.clone());
                        }
                    });
                    ui.end_row();

                    ui.label("Save directory");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.save_dir).desired_width(300.0));
                        if ui.button("Browse…").clicked() {
                            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                self.save_dir = p.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("Server executable");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.exe).desired_width(300.0));
                        if ui.button("Browse…").clicked() {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("Executable", &["exe"])
                                .pick_file()
                            {
                                self.exe = p.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("Launch args\n(one per line)");
                    ui.add(egui::TextEdit::multiline(&mut self.args).desired_rows(3).desired_width(300.0));
                    ui.end_row();
                });

            ui.checkbox(&mut self.allow_writes, "Allow save edits (reserved for a future feature)");
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                if ui.button("Save & apply").clicked() {
                    self.on_save();
                }
                if !self.message.is_empty() {
                    ui.label(&self.message);
                }
            });

            ui.separator();
            ui.label("Log");
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in self.runtime.log_lines() {
                        ui.monospace(line);
                    }
                });
        });
    }
}
