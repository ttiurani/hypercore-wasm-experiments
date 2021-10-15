use random_access_storage::RandomAccess;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/dist/callbacks.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub async fn testing_name() -> Result<JsValue, JsValue>;

    /// Write bytes at an offset to the backend.
    #[wasm_bindgen(catch)]
    pub async fn storage_write(id: &str, offset: u64, data: &[u8]) -> Result<(), JsValue>;

    /// Read a sequence of bytes at an offset from the backend.
    #[wasm_bindgen(catch)]
    async fn storage_read(id: &str, offset: u64, length: u64) -> Result<JsValue, JsValue>;

    /// Delete a sequence of bytes at an offset from the backend.
    #[wasm_bindgen(catch)]
    async fn storage_del(id: &str, offset: u64, length: u64) -> Result<(), JsValue>;

    /// Resize the sequence of bytes, possibly discarding or zero-padding bytes
    /// from the end.
    #[wasm_bindgen(catch)]
    async fn storage_truncate(id: &str, length: u64) -> Result<(), JsValue>;

    /// Get the size of the storage in bytes.
    #[wasm_bindgen(catch)]
    async fn storage_len(id: &str) -> Result<JsValue, JsValue>;

    /// Whether the storage is empty.
    /// For some storage backends it may be cheaper to calculate whether the
    /// storage is empty than to calculate the length.
    #[wasm_bindgen(catch)]
    async fn storage_is_empty(id: &str) -> Result<JsValue, JsValue>;

    /// Flush buffered data on the underlying storage resource.
    #[wasm_bindgen(catch)]
    async fn storage_sync_all(id: &str) -> Result<(), JsValue>;
}

/// Main constructor.
#[derive(Debug)]
pub struct RandomAccessProxy {
    id: String,
    length: u64,
}

impl RandomAccessProxy {
    pub fn new(id: String) -> Self {
        Self { id, length: 0 }
    }
}

#[async_trait::async_trait]
impl RandomAccess for RandomAccessProxy {
    type Error = Box<dyn std::error::Error + Sync + Send>;

    async fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), Self::Error> {
        panic!("Not implemented yet");
    }

    async fn read(&mut self, offset: u64, length: u64) -> Result<Vec<u8>, Self::Error> {
        panic!("Not implemented yet");
    }

    async fn read_to_writer(
        &mut self,
        _offset: u64,
        _length: u64,
        _buf: &mut (impl async_std::io::Write + Send),
    ) -> Result<(), Self::Error> {
        panic!("Not implemented yet");
    }

    async fn del(&mut self, _offset: u64, _length: u64) -> Result<(), Self::Error> {
        panic!("Not implemented yet");
    }

    async fn truncate(&mut self, length: u64) -> Result<(), Self::Error> {
        panic!("Not implemented yet");
    }

    async fn len(&self) -> Result<u64, Self::Error> {
        Ok(self.length)
    }

    async fn is_empty(&mut self) -> Result<bool, Self::Error> {
        Ok(self.length == 0)
    }

    async fn sync_all(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
