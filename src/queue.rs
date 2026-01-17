//! Queue management for audio sources - handles multiple audio sources in sequence

use napi::{Error, Result, Status};
use napi_derive::napi;
use std::sync::{Arc, Mutex};

/// A queue for managing multiple audio sources that play in sequence
#[napi]
pub struct AudioSourceQueue {
    sources: Arc<Mutex<Vec<AudioQueueItem>>>,
    current_index: Arc<Mutex<usize>>,
    is_playing: Arc<Mutex<bool>>,
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
            sources: Arc::new(Mutex::new(Vec::new())),
            current_index: Arc::new(Mutex::new(0)),
            is_playing: Arc::new(Mutex::new(false)),
        }
    }

    /// Add an audio source from a file
    #[napi]
    pub fn add_source(&mut self, file_path: String, title: Option<String>) -> Result<String> {
        let id = format!("source_{}", self.sources.lock().unwrap().len());
        let mut sources = self.sources.lock().unwrap();
        sources.push(AudioQueueItem {
            source_id: id.clone(),
            file_path: Some(file_path),
            buffer: None,
            title,
        });
        Ok(id)
    }

    /// Add an audio source from a buffer
    #[napi]
    pub fn add_buffer(&mut self, buffer: Vec<i16>, title: Option<String>) -> Result<String> {
        let id = format!("source_{}", self.sources.lock().unwrap().len());
        let mut sources = self.sources.lock().unwrap();
        sources.push(AudioQueueItem {
            source_id: id.clone(),
            file_path: None,
            buffer: Some(buffer),
            title,
        });
        Ok(id)
    }

    /// Remove a source by its ID
    #[napi]
    pub fn remove_source(&mut self, source_id: String) -> Result<()> {
        let mut sources = self.sources.lock().unwrap();
        if let Some(pos) = sources.iter().position(|s| s.source_id == source_id) {
            sources.remove(pos);
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Source not found"))
        }
    }

    /// Get a specific source by its ID
    #[napi]
    pub fn get_source(&self, source_id: String) -> Result<AudioQueueItem> {
        let sources = self.sources.lock().unwrap();
        sources
            .iter()
            .find(|s| s.source_id == source_id)
            .cloned()
            .ok_or_else(|| Error::new(Status::InvalidArg, "Source not found"))
    }

    #[napi]
    pub fn get_sources(&self) -> Vec<AudioQueueItem> {
        self.sources.lock().unwrap().clone()
    }

    #[napi]
    pub fn get_length(&self) -> u32 {
        self.sources.lock().unwrap().len() as u32
    }

    #[napi]
    pub fn get_current_index(&self) -> u32 {
        *self.current_index.lock().unwrap() as u32
    }

    #[napi]
    pub fn set_current_index(&self, index: u32) -> Result<()> {
        let len = self.sources.lock().unwrap().len() as u32;
        if index >= len {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Index out of bounds: {} >= {}", index, len),
            ));
        }
        *self.current_index.lock().unwrap() = index as usize;
        Ok(())
    }

    #[napi]
    pub fn clear(&self) {
        *self.sources.lock().unwrap() = Vec::new();
        *self.current_index.lock().unwrap() = 0;
        *self.is_playing.lock().unwrap() = false;
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock().unwrap()
    }

    #[napi]
    pub fn set_playing(&self, playing: bool) {
        *self.is_playing.lock().unwrap() = playing;
    }
}

#[napi]
pub struct SourcesQueueInput {
    queue: Arc<Mutex<AudioSourceQueue>>,
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
            queue: Arc::new(Mutex::new(AudioSourceQueue::new())),
        }
    }

    #[napi]
    pub fn push_file(&self, file_path: String) -> Result<String> {
        let mut queue = self.queue.lock().unwrap();
        queue.add_source(file_path, None)
    }

    #[napi]
    pub fn push_buffer(&self, buffer: Vec<i16>) -> Result<String> {
        let mut queue = self.queue.lock().unwrap();
        queue.add_buffer(buffer, None)
    }

    #[napi]
    pub fn set_title(&self, source_id: String, title: String) -> Result<()> {
        let queue = self.queue.lock().unwrap();
        let mut sources = queue.sources.lock().unwrap();
        if let Some(source) = sources.iter_mut().find(|s| s.source_id == source_id) {
            source.title = Some(title);
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Source not found"))
        }
    }
}

/// Queue output interface - for consuming sources from a queue
#[napi]
pub struct SourcesQueueOutput {
    queue: Arc<Mutex<AudioSourceQueue>>,
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
            queue: Arc::new(Mutex::new(AudioSourceQueue::new())),
        }
    }

    #[napi]
    pub fn peek(&self) -> Result<AudioQueueItem> {
        let queue = self.queue.lock().unwrap();
        let sources = queue.sources.lock().unwrap();
        let idx = *queue.current_index.lock().unwrap();

        if idx >= sources.len() {
            return Err(Error::new(Status::InvalidArg, "Queue is empty"));
        }

        Ok(sources[idx].clone())
    }

    #[napi]
    pub fn pop(&self) -> Result<AudioQueueItem> {
        let queue = self.queue.lock().unwrap();
        let sources = queue.sources.lock().unwrap();
        let mut idx = *queue.current_index.lock().unwrap();

        if idx >= sources.len() {
            return Err(Error::new(Status::InvalidArg, "Queue is empty"));
        }

        let item = sources[idx].clone();
        idx += 1;
        *queue.current_index.lock().unwrap() = idx;
        Ok(item)
    }

    #[napi]
    pub fn has_next(&self) -> bool {
        let queue = self.queue.lock().unwrap();
        let sources = queue.sources.lock().unwrap();
        let idx = *queue.current_index.lock().unwrap();
        idx < sources.len()
    }

    #[napi]
    pub fn get_remaining(&self) -> u32 {
        let queue = self.queue.lock().unwrap();
        let sources = queue.sources.lock().unwrap();
        let idx = *queue.current_index.lock().unwrap();
        (sources.len() - idx) as u32
    }
}

/// Creates a new audio source queue
#[napi]
pub fn queue() -> AudioSourceQueue {
    AudioSourceQueue::new()
}
