use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

#[derive(Debug)]
pub enum RingBufferError {
    Closed,
    ItemTooLarge { item_size: usize, capacity: usize },
    LockPoisoned,
}

impl std::fmt::Display for RingBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "ring buffer is closed"),
            Self::ItemTooLarge {
                item_size,
                capacity,
            } => write!(
                f,
                "ring buffer item is too large: item_size={item_size}, capacity={capacity}"
            ),
            Self::LockPoisoned => write!(f, "ring buffer lock was poisoned"),
        }
    }
}

impl std::error::Error for RingBufferError {}

#[derive(Debug)]
struct State<T> {
    queue: VecDeque<T>,
    bytes_used: usize,
    closed: bool,
}

#[derive(Debug)]
pub struct RingBuffer<T> {
    capacity_bytes: usize,
    size_of: fn(&T) -> usize,
    state: Mutex<State<T>>,
    can_read: Condvar,
    can_write: Condvar,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity_bytes: usize, size_of: fn(&T) -> usize) -> Self {
        Self {
            capacity_bytes,
            size_of,
            state: Mutex::new(State {
                queue: VecDeque::new(),
                bytes_used: 0,
                closed: false,
            }),
            can_read: Condvar::new(),
            can_write: Condvar::new(),
        }
    }

    pub fn push(&self, item: T) -> Result<(), RingBufferError> {
        let item_size = (self.size_of)(&item);
        if item_size > self.capacity_bytes {
            return Err(RingBufferError::ItemTooLarge {
                item_size,
                capacity: self.capacity_bytes,
            });
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| RingBufferError::LockPoisoned)?;
        while !state.closed && state.bytes_used + item_size > self.capacity_bytes {
            state = self
                .can_write
                .wait(state)
                .map_err(|_| RingBufferError::LockPoisoned)?;
        }

        if state.closed {
            return Err(RingBufferError::Closed);
        }

        state.bytes_used += item_size;
        state.queue.push_back(item);
        self.can_read.notify_one();
        Ok(())
    }

    pub fn pop(&self) -> Result<Option<T>, RingBufferError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RingBufferError::LockPoisoned)?;
        while !state.closed && state.queue.is_empty() {
            state = self
                .can_read
                .wait(state)
                .map_err(|_| RingBufferError::LockPoisoned)?;
        }

        let Some(item) = state.queue.pop_front() else {
            return Ok(None);
        };

        state.bytes_used -= (self.size_of)(&item);
        self.can_write.notify_one();
        Ok(Some(item))
    }

    pub fn close(&self) -> Result<(), RingBufferError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RingBufferError::LockPoisoned)?;
        state.closed = true;
        self.can_read.notify_all();
        self.can_write.notify_all();
        Ok(())
    }

    pub fn len(&self) -> Result<usize, RingBufferError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| RingBufferError::LockPoisoned)?
            .queue
            .len())
    }

    pub fn is_empty(&self) -> Result<bool, RingBufferError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| RingBufferError::LockPoisoned)?
            .queue
            .is_empty())
    }

    pub fn bytes_used(&self) -> Result<usize, RingBufferError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| RingBufferError::LockPoisoned)?
            .bytes_used)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn preserves_fifo_order() {
        let buffer = RingBuffer::new(16, |item: &Vec<u8>| item.len());
        buffer.push(vec![1, 2]).unwrap();
        buffer.push(vec![3, 4]).unwrap();

        assert_eq!(buffer.pop().unwrap(), Some(vec![1, 2]));
        assert_eq!(buffer.pop().unwrap(), Some(vec![3, 4]));
    }

    #[test]
    fn unblocks_reader_when_closed() {
        let buffer = Arc::new(RingBuffer::new(16, |item: &Vec<u8>| item.len()));
        let reader = Arc::clone(&buffer);
        let handle = thread::spawn(move || reader.pop().unwrap());

        buffer.close().unwrap();

        assert_eq!(handle.join().unwrap(), None);
    }
}
