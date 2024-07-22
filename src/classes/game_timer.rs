use crate::{
    bib::types::{GameTimer, Task},
    CloneableCallback,
};
use std::sync::{Arc, Mutex};

impl GameTimer {
    pub fn new() -> Self {
        GameTimer {
            ticks: 0,
            time_accumulated: 0,
            tasks: Vec::new(),
        }
    }

    pub fn tick(&mut self, delta_time: u64) {
        self.ticks += 1;
        self.time_accumulated += delta_time;
        self.actualizar_tareas();
    }

    pub fn set_timeout<F>(&mut self, callback: F, delay_ms: u64)
    where
        F: Fn() + 'static + Clone + Send + Sync,
    {
        let execute_on_ms = self.time_accumulated + delay_ms;
        let callback_arc = Arc::new(Mutex::new(Some(callback)));
        self.tasks.push(Task {
            execute_on_ms,
            callback: CloneableCallback::new(move || {
                if let Some(callback) = callback_arc.lock().unwrap().take() {
                    callback();
                }
            }),
        });
    }

    fn actualizar_tareas(&mut self) {
        let ready_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| task.execute_on_ms <= self.time_accumulated)
            .cloned()
            .collect();
        for task in ready_tasks {
            (task.callback)();
        }

        self.tasks
            .retain(|task| task.execute_on_ms > self.time_accumulated);
    }
}
