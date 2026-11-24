//! puts a finished picture in the window as plain pixels, the way each system has for it

use baseview::{HandlerError, WindowContext};

/// the window as something pixels can be put in
pub(super) struct Screen {
  #[cfg(target_os = "windows")]
  window: windows_sys::Win32::Foundation::HWND,
  #[cfg(not(target_os = "windows"))]
  surface: softbuffer::Surface<baseview::PlatformHandle, baseview::PlatformHandle>,
  /// the picture last shown, which goes in the window again whenever the system wants the
  /// window painted and nothing on the panel has changed
  kept: Option<Kept>,
}

/// a picture of `size` pixels: its rows from the top, each `row` bytes long and starting
/// with a pixel's blue, green, red and one more byte for every pixel across
struct Kept {
  size: [u32; 2],
  row: u32,
  pixels: Vec<u8>,
}

impl Screen {
  /// shows a picture over the whole window and keeps it. `pixels` holds its rows as
  /// [`Kept`] has them
  pub(super) fn show(
    &mut self,
    size: [u32; 2],
    row: u32,
    pixels: &[u8],
  ) -> Result<(), HandlerError> {
    assert!(row >= size[0] * 4, "a row holds the picture's width");
    let mut held = self.kept.take().map_or_else(Vec::new, |kept| kept.pixels);
    held.clear();
    held.extend_from_slice(&pixels[..row as usize * size[1] as usize]);
    self.kept = Some(Kept {
      size,
      row,
      pixels: held,
    });
    self.again()
  }
}

#[cfg(target_os = "windows")]
impl Screen {
  pub(super) fn new(context: &WindowContext) -> Result<Self, HandlerError> {
    use wgpu::rwh::{HasWindowHandle, RawWindowHandle};

    let RawWindowHandle::Win32(window) = context.window_handle()?.as_raw() else {
      return Err(HandlerError::from_boxed(
        "the window is not a windows one".into(),
      ));
    };
    Ok(Self {
      window: window.hwnd.get() as _,
      kept: None,
    })
  }

  /// puts the picture last shown in the window again, if there has been one
  pub(super) fn again(&mut self) -> Result<(), HandlerError> {
    use windows_sys::Win32::Graphics::Gdi::{
      BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDC, ReleaseDC, SetDIBitsToDevice,
    };

    let Some(Kept { size, row, pixels }) = &self.kept else {
      return Ok(());
    };
    let ([width, height], row) = (*size, *row);
    let header = BITMAPINFOHEADER {
      biSize: size_of::<BITMAPINFOHEADER>() as u32,
      // the rows as they are, with what is past the picture's width left out when shown
      biWidth: (row / 4) as i32,
      // a height below zero has the first row at the top
      biHeight: -(height as i32),
      biPlanes: 1,
      biBitCount: 32,
      biCompression: BI_RGB,
      biSizeImage: 0,
      biXPelsPerMeter: 0,
      biYPelsPerMeter: 0,
      biClrUsed: 0,
      biClrImportant: 0,
    };
    // SAFETY: the window is the one baseview made and is alive for as long as its handler,
    // which owns this. the header describes rows of `row` bytes, `height` of them, which is
    // what `pixels` was kept as, and gdi reads no further
    let rows = unsafe {
      let info = BITMAPINFO {
        bmiHeader: header,
        bmiColors: [std::mem::zeroed()],
      };
      let dc = GetDC(self.window);
      if dc.is_null() {
        return Err(HandlerError::from_boxed(
          "the window has nothing to draw on".into(),
        ));
      }
      let rows = SetDIBitsToDevice(
        dc,
        0,
        0,
        width,
        height,
        0,
        0,
        0,
        height,
        pixels.as_ptr().cast(),
        &info,
        DIB_RGB_COLORS,
      );
      ReleaseDC(self.window, dc);
      rows
    };
    if rows == 0 {
      return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
  }
}

#[cfg(not(target_os = "windows"))]
impl Screen {
  pub(super) fn new(context: &WindowContext) -> Result<Self, HandlerError> {
    let window = context.platform_handle();
    let display = softbuffer::Context::new(window.clone())?;
    Ok(Self {
      surface: softbuffer::Surface::new(&display, window)?,
      kept: None,
    })
  }

  /// puts the picture last shown in the window again, if there has been one
  pub(super) fn again(&mut self) -> Result<(), HandlerError> {
    use std::num::NonZeroU32;

    let Some(Kept { size, row, pixels }) = &self.kept else {
      return Ok(());
    };
    let [Some(width), Some(height)] = size.map(NonZeroU32::new) else {
      return Ok(());
    };
    self.surface.resize(width, height)?;
    let mut shown = self.surface.buffer_mut()?;
    let rows = shown.chunks_exact_mut(size[0] as usize);
    for (shown, drawn) in rows.zip(pixels.chunks_exact(*row as usize)) {
      for (shown, &[blue, green, red, _]) in shown.iter_mut().zip(drawn.as_chunks::<4>().0) {
        // softbuffer has a pixel as a number with blue lowest and nothing above red
        *shown = u32::from_le_bytes([blue, green, red, 0]);
      }
    }
    shown.present()?;
    Ok(())
  }
}
