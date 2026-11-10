use std::ops::Deref;

pub(crate) struct Revised<T> {
    value: T,
    revision: u64,
}

impl<T> Revised<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value, revision: 0 }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn set(&mut self, value: T) {
        self.value = value;
        self.revision += 1;
    }

    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.revision += 1;
        &mut self.value
    }
}

impl<T> Deref for Revised<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: Default> Default for Revised<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests;
