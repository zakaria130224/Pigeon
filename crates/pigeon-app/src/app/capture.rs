//! `--screenshot`, and scripted input for testing the real window.

use super::*;
use std::sync::atomic::Ordering;

impl App {
    /// Test aid for screenshot runs: `PIGEON_QA_EVENTS` holds steps like
    /// `20 click 680 153`, `30 down 10 10`, `35 up 10 10`, `40 text {{ba` and `50 key Enter`, one per line, each with its frame.
    #[cfg(feature = "qa")]
    pub(super) fn scripted_input(&mut self, input: &mut egui::RawInput) {
        let Some(shot) = &self.shot else { return };
        let Ok(steps) = std::env::var("PIGEON_QA_EVENTS") else { return };
        for step in steps.lines().map(str::trim) {
            let mut parts = step.splitn(3, ' ');
            let (Some(frame), Some(kind), rest) = (parts.next(), parts.next(), parts.next().unwrap_or("")) else { continue };
            if frame.parse::<u32>().ok() != Some(shot.frames) {
                continue;
            }
            match kind {
                "click" | "down" | "up" | "move" => {
                    let mut xy = rest.split(' ').filter_map(|n| n.parse::<f32>().ok());
                    let pos = egui::pos2(xy.next().unwrap_or(0.0), xy.next().unwrap_or(0.0));
                    input.events.push(egui::Event::PointerMoved(pos));
                    let presses: &[bool] = match kind {
                        "click" => &[true, false],
                        "down" => &[true],
                        "up" => &[false],
                        _ => &[],
                    };
                    for pressed in presses {
                        input.events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: *pressed,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
                "text" => input.events.push(egui::Event::Text(rest.to_string())),
                "key" => {
                    if let Some(key) = egui::Key::from_name(rest) {
                        for pressed in [true, false] {
                            input.events.push(egui::Event::Key {
                                key,
                                physical_key: None,
                                pressed,
                                repeat: false,
                                modifiers: egui::Modifiers::NONE,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Test aid for screenshot runs: `PIGEON_QA_PROJECTS` lists recent folders, one per line, and
    /// `PIGEON_QA_SYNC` fills Team Sync with sample changes (`conflict` adds a merge in progress).
    #[cfg(feature = "qa")]
    pub(super) fn qa_sample(&mut self) {
        for path in std::env::var("PIGEON_QA_PROJECTS").unwrap_or_default().lines().rev() {
            self.settings.remember_project(std::path::Path::new(path));
        }
        let Ok(sync) = std::env::var("PIGEON_QA_SYNC") else { return };
        let change = |code: &str, path: &str| (code.to_string(), path.to_string());
        let commit = |hash: &str, subject: &str| git::Commit {
            hash: hash.into(),
            author: "Sam".into(),
            when: "2 hours ago".into(),
            subject: subject.into(),
        };
        self.gstat.ahead = 1;
        self.gstat.behind = 2;
        self.gstat.branches = vec!["main".into(), "payments".into()];
        self.gstat.remote_url = "git@github.com:team/api-collections.git".into();
        self.gstat.upstream = Some("origin/main".into());
        self.gstat.user_name = "Sam".into();
        self.gstat.changes = vec![
            change(" M", "collections/Sample API/Users/List users.request.json"),
            change("??", "collections/Sample API/Users/A request with a very long name that needs to be cut short.request.json"),
            change(" D", "environments/Staging.environment.json"),
        ];
        self.gstat.log =
            vec![commit("a1b2c3d", "Add the users folder"), commit("e4f5a6b", "A commit message long enough that it has to be cut short")];
        self.remote_input = self.gstat.remote_url.clone();
        if sync == "conflict" {
            self.gstat.merging = true;
            self.gstat.conflicts = vec![change("UU", "collections/Sample API/Users/Get user.request.json")];
        }
    }

    pub(super) fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        if shot.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        shot.frames += 1;
        if shot.frames >= 120 && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            let done = shot.done.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for px in &img.pixels {
                    buf.extend_from_slice(&px.to_srgba_unmultiplied());
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = image::save_buffer(&path, &buf, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                    eprintln!("could not save screenshot: {}", e);
                }
                done.store(true, Ordering::SeqCst);
                ctx2.request_repaint();
            });
        }
        if shot.frames > 900 {
            eprintln!("screenshot was not delivered in time");
            std::process::exit(1);
        }
    }
}
