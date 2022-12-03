use std::{
    collections::VecDeque,
    iter::Sum,
    ops::Div,
    time::{Duration, Instant},
};

pub struct Averager<T> {
    data: VecDeque<(Instant, T)>,
    duration: Duration,
}

impl<T> Averager<T> {
    pub fn new(duration: Duration) -> Self {
        Self {
            data: VecDeque::with_capacity(256),
            duration,
        }
    }
}

impl<T> Averager<T>
where
    T: Sum + Copy + Clone + Div<u32, Output = T> + Default,
{
    pub fn feed(&mut self, data: T) {
        let now = Instant::now();
        let some_time_ago = now - self.duration;

        while self.data.front().map_or(false, |(t, _)| *t < some_time_ago) {
            self.data.pop_front();
        }
        self.data.push_back((now, data));
    }

    pub fn average(&self) -> T {
        let len = self.data.len();
        if len > 0 {
            self.data.iter().map(|(_, d)| *d).sum::<T>() / len as u32
        } else {
            T::default()
        }
    }

    pub fn ticks(&self) -> usize {
        self.data.len()
    }
}
