//! Sending requests and running collections.

use super::*;

impl App {
    pub(super) fn send_tab(&mut self, tab: &mut ReqTab) {
        let job = self.job_for(&tab.req, tab.origin.as_deref(), 1);
        tab.rx = Some(scripting::spawn(job, self.scopes_for(tab.origin.as_deref())));
        tab.resp = None;
    }

    pub(super) fn poll_responses(&mut self) {
        let mut done: Vec<(usize, Outcome)> = vec![];
        let mut runs: Vec<(String, Box<Outcome>, String)> = vec![];
        for (i, t) in self.tabs.iter_mut().enumerate() {
            match &mut t.kind {
                TabKind::Request(rt) => match rt.rx.as_ref().map(|rx| rx.try_recv()) {
                    Some(Ok(outcome)) => {
                        rt.rx = None;
                        done.push((i, outcome));
                    }
                    Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                        rt.rx = None;
                        rt.resp = Some(ResponseData { error: Some("The request stopped unexpectedly.".into()), ..Default::default() });
                    }
                    _ => {}
                },
                TabKind::Runner(run) => {
                    let mut finished = false;
                    if let Some(rx) = &run.rx {
                        while let Ok(m) = rx.try_recv() {
                            match m {
                                RunMsg::Result(r, outcome) => {
                                    runs.push((run.coll.clone(), outcome, format!("Runner · {}", r.name)));
                                    run.results.push(r);
                                }
                                RunMsg::Done => {
                                    finished = true;
                                    break;
                                }
                            }
                        }
                    }
                    if finished {
                        run.rx = None;
                    }
                }
                _ => {}
            }
        }
        for (i, outcome) in done {
            let (origin, req) = match &self.tabs[i].kind {
                TabKind::Request(rt) => (rt.origin.clone(), rt.req.clone()),
                _ => continue,
            };
            self.record_outcome(origin.as_deref(), &outcome, &req.name);
            let r = outcome.resp;
            self.history.insert(0, HistoryEntry { at_ms: now_ms(), request: clean_request(&req), status: r.status, time_ms: r.time_ms });
            self.history.truncate(300);
            if !self.no_disk {
                settings::save_history(&self.history);
            }
            if let TabKind::Request(rt) = &mut self.tabs[i].kind {
                if r.tests.iter().any(|t| !t.1) || r.script_error.is_some() {
                    rt.resp_view = RespView::Tests;
                }
                rt.resp = Some(r);
            }
        }
        for (coll, outcome, source) in runs {
            self.record_outcome(Some(&coll), &outcome, &source);
        }
    }

    pub(super) fn start_run(&self, run: &mut RunnerTab) {
        let Some(c) = self.coll(&run.coll) else { return };
        let mut scopes = self.scopes_for(Some(&run.coll));
        let mut reqs = vec![];
        flatten_requests(&c.items, &mut reqs);
        let iterations = run.iterations.max(1) as usize;
        let jobs: Vec<Job> = reqs.iter().map(|r| self.job_for(r, Some(&run.coll), iterations)).collect();
        let (tx, rx) = std::sync::mpsc::channel();
        let delay = run.delay_ms;
        std::thread::spawn(move || {
            'run: for iteration in 0..iterations {
                let mut at = 0;
                while let Some(job) = jobs.get(at) {
                    let mut job = job.clone();
                    job.iteration = iteration;
                    let outcome = scripting::run(&job, &mut scopes);
                    let r = &outcome.resp;
                    let sent = r.sent.as_ref();
                    let result = RunResult {
                        name: job.req.name.clone(),
                        method: sent.map(|p| p.method.clone()).unwrap_or_default(),
                        url: sent.map(|p| p.url.clone()).unwrap_or_default(),
                        status: r.status,
                        time_ms: r.time_ms,
                        error: r.error.clone().or_else(|| r.script_error.clone()),
                        tests: r.tests.clone(),
                    };
                    let next = outcome.next.clone();
                    // Fails once the run was stopped and nobody is listening.
                    if tx.send(RunMsg::Result(result, Box::new(outcome))).is_err() {
                        return;
                    }
                    if delay > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay));
                    }
                    at = match next {
                        None => at + 1,
                        Some(None) => break 'run,
                        Some(Some(name)) => match jobs.iter().position(|j| j.req.name == name) {
                            Some(i) => i,
                            None => break 'run,
                        },
                    };
                }
            }
            let _ = tx.send(RunMsg::Done);
        });
        run.results.clear();
        run.rx = Some(rx);
        run.started = true;
    }
}
