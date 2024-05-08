use crate::lib::types::{GameTimer, Task};

impl GameTimer {
    fn new() -> Self {
        GameTimer {
            ticks: 0,
            time_accumulated: 0.0,
            tasks: Vec::new(),
        }
    }

    pub fn tick(&mut self, delta_time: f64) {
        self.ticks += 1;
        self.time_accumulated += delta_time;
        self.update_tasks();
    }

    pub fn set_timeout<F>(&mut self, callback: F, delay_ms: f64)
    where
        F: Fn() + 'static,
    {
        let execute_on_ms = self.time_accumulated + delay_ms;
        self.tasks.push(Task {
            execute_on_ms,
            callback: Box::new(callback),
        });
    }

    fn update_tasks(&mut self) {
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
