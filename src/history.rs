use serde::{Deserialize, Serialize};

const HISTORY_SIZE: usize = 32;

#[derive(Serialize, Deserialize, Debug)]
pub struct History<T> {
    data: [Option<T>; HISTORY_SIZE],
    pointer: usize,
}

#[allow(dead_code)]
impl<T> History<T> {
    pub fn new() -> Self {
        let mut data = vec![];
        for _ in 0..HISTORY_SIZE {
            data.push(None);
        }

        Self {
            data: data
                .try_into()
                .unwrap_or_else(|_| panic!("Failed to generate history array")),
            pointer: 0,
        }
    }

    pub fn insert(&mut self, item: T) {
        self.data[self.pointer] = Some(item);
        self.pointer += 1;
        if self.pointer > self.data.len() - 1 {
            self.pointer = 0;
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.data[index].as_ref()
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data[index].as_mut()
    }

    pub fn get_all(&self) -> Vec<&T> {
        let mut result = vec![];
        let mut c = self.pointer;
        for _ in 0..HISTORY_SIZE {
            if let Some(k) = self.data[c].as_ref() {
                result.push(k);
            }

            c += 1;
            if c > self.data.len() - 1 {
                c = 0;
            }
        }

        result
    }

    pub fn get_count(&self) -> usize {
        self.pointer
    }
}