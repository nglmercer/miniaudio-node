//! Queue management for audio sources - handles multiple audio sources in sequence

use napi::{Error, Result, Status};
use napi_derive::napi;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A queue for managing multiple audio sources that play in sequence.
///
/// The state is held behind an `Arc` so producer and consumer wrappers can be
/// explicitly created for the same queue.
#[napi]
pub struct AudioSourceQueue {
    state: Arc<QueueState>,
}

struct QueueState {
    sources: Mutex<Vec<AudioQueueItem>>,
    current_index: Mutex<usize>,
    is_playing: Mutex<bool>,
    next_id: AtomicU64,
}

impl QueueState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            sources: Mutex::new(Vec::new()),
            current_index: Mutex::new(0),
            is_playing: Mutex::new(false),
            next_id: AtomicU64::new(0),
        })
    }

    fn next_source_id(&self) -> String {
        format!("source_{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for AudioSourceQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[napi(object)]
#[derive(Clone)]
pub struct AudioQueueItem {
    pub source_id: String,
    pub file_path: Option<String>,
    pub buffer: Option<Vec<i16>>,
    pub title: Option<String>,
}

#[napi]
impl AudioSourceQueue {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            state: QueueState::new(),
        }
    }

    fn from_state(state: Arc<QueueState>) -> Self {
        Self { state }
    }

    /// Add an audio source from a file.
    #[napi]
    pub fn add_source(&self, file_path: String, title: Option<String>) -> Result<String> {
        let id = self.state.next_source_id();
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(AudioQueueItem {
                source_id: id.clone(),
                file_path: Some(file_path),
                buffer: None,
                title,
            });
        Ok(id)
    }

    /// Add an audio source from a buffer.
    #[napi]
    pub fn add_buffer(&self, buffer: Vec<i16>, title: Option<String>) -> Result<String> {
        let id = self.state.next_source_id();
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(AudioQueueItem {
                source_id: id.clone(),
                file_path: None,
                buffer: Some(buffer),
                title,
            });
        Ok(id)
    }

    /// Remove a source by its ID.
    #[napi]
    pub fn remove_source(&self, source_id: String) -> Result<()> {
        let mut sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        let Some(position) = sources.iter().position(|s| s.source_id == source_id) else {
            return Err(Error::new(Status::InvalidArg, "Source not found"));
        };

        sources.remove(position);
        let mut current_index = self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if position < *current_index {
            *current_index -= 1;
        }
        *current_index = (*current_index).min(sources.len());
        if sources.is_empty() {
            *self
                .state
                .is_playing
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = false;
        }
        Ok(())
    }

    /// Get a specific source by its ID.
    #[napi]
    pub fn get_source(&self, source_id: String) -> Result<AudioQueueItem> {
        let sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        sources
            .iter()
            .find(|s| s.source_id == source_id)
            .cloned()
            .ok_or_else(|| Error::new(Status::InvalidArg, "Source not found"))
    }

    #[napi]
    pub fn get_sources(&self) -> Vec<AudioQueueItem> {
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    #[napi]
    pub fn get_length(&self) -> u32 {
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len() as u32
    }

    #[napi]
    pub fn get_current_index(&self) -> u32 {
        *self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner()) as u32
    }

    #[napi]
    pub fn set_current_index(&self, index: u32) -> Result<()> {
        let len = self
            .state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len() as u32;
        if index >= len {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Index out of bounds: {} >= {}", index, len),
            ));
        }
        *self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = index as usize;
        Ok(())
    }

    #[napi]
    pub fn clear(&self) {
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        *self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = 0;
        *self
            .state
            .is_playing
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = false;
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        *self
            .state
            .is_playing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[napi]
    pub fn set_playing(&self, playing: bool) {
        *self
            .state
            .is_playing
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = playing;
    }
}

#[napi]
pub struct SourcesQueueInput {
    state: Arc<QueueState>,
}

impl Default for SourcesQueueInput {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl SourcesQueueInput {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            state: QueueState::new(),
        }
    }

    /// Create a producer connected to an existing queue.
    #[napi(factory)]
    pub fn from_queue(queue: &AudioSourceQueue) -> Self {
        Self {
            state: queue.state.clone(),
        }
    }

    #[napi]
    pub fn push_file(&self, file_path: String) -> Result<String> {
        AudioSourceQueue::from_state(self.state.clone()).add_source(file_path, None)
    }

    #[napi]
    pub fn push_buffer(&self, buffer: Vec<i16>) -> Result<String> {
        AudioSourceQueue::from_state(self.state.clone()).add_buffer(buffer, None)
    }

    #[napi]
    pub fn set_title(&self, source_id: String, title: String) -> Result<()> {
        let mut sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(source) = sources.iter_mut().find(|s| s.source_id == source_id) {
            source.title = Some(title);
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Source not found"))
        }
    }
}

/// Queue output interface - for consuming sources from a queue.
#[napi]
pub struct SourcesQueueOutput {
    state: Arc<QueueState>,
}

impl Default for SourcesQueueOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl SourcesQueueOutput {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            state: QueueState::new(),
        }
    }

    /// Create a consumer connected to an existing queue.
    #[napi(factory)]
    pub fn from_queue(queue: &AudioSourceQueue) -> Self {
        Self {
            state: queue.state.clone(),
        }
    }

    #[napi]
    pub fn peek(&self) -> Result<AudioQueueItem> {
        let sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        let index = *self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        sources
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new(Status::InvalidArg, "Queue is empty"))
    }

    #[napi]
    pub fn pop(&self) -> Result<AudioQueueItem> {
        let sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        let mut index = self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let Some(item) = sources.get(*index).cloned() else {
            return Err(Error::new(Status::InvalidArg, "Queue is empty"));
        };
        *index += 1;
        Ok(item)
    }

    #[napi]
    pub fn has_next(&self) -> bool {
        let sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        let index = *self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        index < sources.len()
    }

    #[napi]
    pub fn get_remaining(&self) -> u32 {
        let sources = self.state.sources.lock().unwrap_or_else(|e| e.into_inner());
        let index = *self
            .state
            .current_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sources.len().saturating_sub(index) as u32
    }
}

/// Creates a new audio source queue.
#[napi]
pub fn queue() -> AudioSourceQueue {
    AudioSourceQueue::new()
}
