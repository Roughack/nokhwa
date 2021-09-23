/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::{
    Camera, CameraFormat, CameraInfo, CaptureAPIBackend, FrameFormat, NokhwaError, Resolution,
};
use image::{ImageBuffer, Rgb};
use parking_lot::FairMutex;
use std::{collections::HashMap, sync::Arc, thread::JoinHandle};

pub struct ThreadedCamera {
    camera: Arc<FairMutex<Camera>>,
    thread_handle: JoinHandle<()>,
    frame_callback: Arc<FairMutex<Option<fn(ImageBuffer<Rgb<u8>, Vec<u8>>)>>>,
}

impl ThreadedCamera {
    /// Create a new camera from an `index`, `format`, and `backend`. `format` can be `None`.
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn new(
        index: usize,
        format: Option<CameraFormat>,
        backend: CaptureAPIBackend,
    ) -> Result<Self, NokhwaError> {
        let camera = Arc::new(FairMutex::new(Camera::new(index, format, backend)?));
        let frame_callback = Arc::new(FairMutex::new(None));

        let thread_handle =
            std::thread::spawn(|| camera_frame_thread_loop(camera.clone(), frame_callback.clone()));

        Ok(ThreadedCamera {
            camera,
            thread_handle,
            frame_callback,
        })
    }

    /// Create a new `Camera` from raw values.
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn new_with(
        index: usize,
        width: u32,
        height: u32,
        fps: u32,
        fourcc: FrameFormat,
        backend: CaptureAPIBackend,
    ) -> Result<Self, NokhwaError> {
        let camera_format = CameraFormat::new_from(width, height, fourcc, fps);
        ThreadedCamera::new(index, Some(camera_format), backend)
    }

    /// Gets the current Camera's index.
    #[must_use]
    pub fn index(&self) -> usize {
        self.camera.lock().index()
    }

    /// Sets the current Camera's index. Note that this re-initializes the camera.
    /// # Errors
    /// The Backend may fail to initialize.
    pub fn set_index(&mut self, new_idx: usize) -> Result<(), NokhwaError> {
        self.camera.lock().set_index(new_idx)
    }

    /// Gets the current Camera's backend
    #[must_use]
    pub fn backend(&self) -> CaptureAPIBackend {
        self.camera.lock().backend()
    }

    /// Sets the current Camera's backend. Note that this re-initializes the camera.
    /// # Errors
    /// The new backend may not exist or may fail to initialize the new camera.
    pub fn set_backend(&mut self, new_backend: CaptureAPIBackend) -> Result<(), NokhwaError> {
        self.camera.lock().set_backend(new_backend)
    }

    /// Gets the camera information such as Name and Index as a [`CameraInfo`].
    #[must_use]
    pub fn info(&self) -> &CameraInfo {
        self.camera.lock().info()
    }

    /// Gets the current [`CameraFormat`].
    #[must_use]
    pub fn camera_format(&self) -> CameraFormat {
        self.camera.lock().camera_format()
    }

    /// Will set the current [`CameraFormat`]
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new camera format, this will return an error.
    pub fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        self.camera.lock().set_camera_format(new_fmt)
    }

    /// A hashmap of [`Resolution`]s mapped to framerates
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        self.camera.lock().compatible_list_by_resolution(fourcc)
    }

    /// A Vector of compatible [`FrameFormat`]s.
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        self.camera.lock().compatible_fourcc()
    }

    /// Gets the current camera resolution (See: [`Resolution`], [`CameraFormat`]).
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.camera.lock().resolution()
    }

    /// Will set the current [`Resolution`]
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new resolution, this will return an error.
    pub fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        self.camera.lock().set_resolution(new_res)
    }

    /// Gets the current camera framerate (See: [`CameraFormat`]).
    #[must_use]
    pub fn frame_rate(&self) -> u32 {
        self.camera.lock().frame_rate()
    }

    /// Will set the current framerate
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new framerate, this will return an error.
    pub fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        self.camera.lock().set_frame_rate(new_fps)
    }

    /// Gets the current camera's frame format (See: [`FrameFormat`], [`CameraFormat`]).
    #[must_use]
    pub fn frame_format(&self) -> FrameFormat {
        self.camera.lock().frame_format()
    }

    /// Will set the current [`FrameFormat`]
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new frame format, this will return an error.
    pub fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
        self.camera.lock().set_frame_format(fourcc)
    }

    /// Will open the camera stream with set parameters. This will be called internally if you try and call [`frame()`](CaptureBackendTrait::frame()) before you call [`open_stream()`](CaptureBackendTrait::open_stream()).
    /// The callback will be called every frame.
    /// # Errors
    /// If the specific backend fails to open the camera (e.g. already taken, busy, doesn't exist anymore) this will error.
    pub fn open_stream(
        &mut self,
        callback: fn(ImageBuffer<Rgb<u8>, Vec<u8>>),
    ) -> Result<(), NokhwaError> {
        *self.frame_callback.lock() = Some(callback);
        self.camera.lock().open_stream()
    }

    pub fn set_callback(&mut self, callback: fn(ImageBuffer<Rgb<u8>, Vec<u8>>)) {
        *self.frame_callback.lock() = Some(callback);
    }

    /// Checks if stream if open. If it is, it will return true.
    pub fn is_stream_open(&self) -> bool {
        self.camera.lock().is_stream_open()
    }

    /// Will drop the stream.
    /// # Errors
    /// Please check the `Quirks` section of each backend.
    pub fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        self.camera.lock().stop_stream()
    }
}

fn camera_frame_thread_loop(
    camera: Arc<FairMutex<Camera>>,
    callback: Arc<FairMutex<Option<fn(ImageBuffer<Rgb<u8>, Vec<u8>>)>>>,
) {
    loop {
        if let Ok(img) = camera.lock().frame() {
            if let Some(cb) = callback.lock() {
                cb(img)
            }
        }
    }
}
