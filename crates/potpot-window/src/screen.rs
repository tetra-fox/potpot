//! The way each system puts a finished picture in the window as plain pixels.

use baseview::{HandlerError, WindowContext};

/// The window as a place to put pixels.
pub(super) struct Screen {
  #[cfg(target_os = "windows")]
  window: windows_sys::Win32::Foundation::HWND,
  #[cfg(not(target_os = "windows"))]
  surface: softbuffer::Surface<baseview::PlatformHandle, baseview::PlatformHandle>,
  /// The picture last shown.
  ///
  /// It goes in the window again whenever the system wants the window painted and nothing on
  /// the panel has changed.
  #[cfg(any(target_os = "windows", target_os = "macos"))]
  kept: Option<Kept>,
  /// The bytes in a row of the picture last shown.
  ///
  /// The row is as it was read back.
  #[cfg(not(any(target_os = "windows", target_os = "macos")))]
  row: u32,
}

/// A picture of `size` pixels.
///
/// `pixels` holds its rows from the top. Each row is `row` bytes long and starts with four
/// bytes for every pixel across: blue, green, red and one more.
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(super) struct Kept {
  pub(super) size: [u32; 2],
  pub(super) row: u32,
  pub(super) pixels: Vec<u8>,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl Kept {
  /// Takes the parts of a picture that differ from this one.
  ///
  /// The two are of the same size. Each part is `[left, top, width, height]` in the picture's
  /// pixels and lies on it. `pixels` holds the picture's rows from `top` on as this has them, of
  /// which only the parts are read.
  pub(super) fn patch(&mut self, parts: &[[u32; 4]], top: u32, pixels: &[u8]) {
    let row = self.row as usize;
    for [left, part_top, width, height] in parts.iter().map(|part| part.map(|side| side as usize)) {
      for down in part_top..part_top + height {
        let from = (down - top as usize) * row + left * 4;
        let to = down * row + left * 4;
        self.pixels[to..to + width * 4].copy_from_slice(&pixels[from..from + width * 4]);
      }
    }
  }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl Screen {
  /// Shows a picture over the whole window and keeps it.
  ///
  /// `pixels` holds its rows as [`Kept`] has them.
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
    self.put(None, true)
  }

  /// Shows and keeps a picture that is new in parts.
  ///
  /// The picture is of the size of the one shown last and differs from it in `parts`. Those,
  /// `top` and `pixels` are as [`Kept::patch`] takes them.
  ///
  /// `all` is whether the window may have lost any of the picture shown last. If it has not,
  /// the parts are all that has to go in it.
  ///
  /// Returns whether the picture is in the window, which it always is here.
  ///
  /// # Panics
  ///
  /// Panics if no picture has been shown.
  pub(super) fn patch(
    &mut self,
    parts: &[[u32; 4]],
    top: u32,
    pixels: &[u8],
    all: bool,
  ) -> Result<bool, HandlerError> {
    let kept = self.kept.as_mut().expect("a picture was shown before");
    kept.patch(parts, top, pixels);
    self.put(Some(parts), all)?;
    Ok(true)
  }

  /// Puts the picture last shown in the window again.
  ///
  /// With none shown yet it does nothing. Returns whether the window has its picture, which it
  /// always has here.
  pub(super) fn again(&mut self) -> Result<bool, HandlerError> {
    self.put(Some(&[]), true)?;
    Ok(true)
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

  /// Puts the kept picture in the window, all of it.
  ///
  /// baseview draws every frame here for a paint message, and the system takes the window as
  /// painted once the message is answered, whatever was put in it. So all of it goes in each
  /// time, whichever parts are new.
  fn put(&mut self, _new: Option<&[[u32; 4]]>, _all: bool) -> Result<(), HandlerError> {
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
      return Err(HandlerError::from_boxed(
        "the picture could not be put in the window".into(),
      ));
    }
    Ok(())
  }
}

/// The byte that makes a pixel opaque.
///
/// baseview's X11 window has a 32 bit visual, where a compositor reads that byte of a pixel as
/// how opaque the pixel is.
#[cfg(not(target_os = "windows"))]
const OPAQUE: u32 = 0xff00_0000;

/// Writes parts of a picture into the pixels of a surface.
///
/// `shown` is the surface's rows from the top, `wide` pixels each. Each part is
/// `[left, top, width, height]` in the picture's pixels and lies on it. `pixels` holds the
/// picture's rows from `top` on as they are read back: `row` bytes each, which start with four
/// bytes for every pixel across, blue, green, red and one more. Only the parts are read.
///
/// softbuffer has a pixel as a number with blue lowest, and each one written is made opaque.
#[cfg(not(target_os = "windows"))]
pub(super) fn write(
  shown: &mut [u32],
  wide: usize,
  parts: &[[u32; 4]],
  top: u32,
  pixels: &[u8],
  row: usize,
) {
  for [left, part_top, width, height] in parts.iter().map(|part| part.map(|side| side as usize)) {
    for down in part_top..part_top + height {
      let drawn = &pixels[(down - top as usize) * row + left * 4..][..width * 4];
      let shown = &mut shown[down * wide + left..][..width];
      for (shown, drawn) in shown.iter_mut().zip(drawn.as_chunks::<4>().0) {
        *shown = u32::from_le_bytes(*drawn) | OPAQUE;
      }
    }
  }
}

#[cfg(target_os = "macos")]
impl Screen {
  pub(super) fn new(context: &WindowContext) -> Result<Self, HandlerError> {
    let window = context.platform_handle();
    let display = softbuffer::Context::new(window.clone())?;
    Ok(Self {
      surface: softbuffer::Surface::new(&display, window)?,
      kept: None,
    })
  }

  /// Puts the kept picture in the window.
  ///
  /// `new` is the parts of it that differ from what was put there last, or `None` for any part,
  /// and `all` whether the window may have lost any of that. Nothing is put if nothing is new
  /// or lost.
  ///
  /// softbuffer 0.4.8 hands out an empty surface for every picture here
  /// (`CGImpl::buffer_mut` in its `backends/cg.rs`), so all of the picture is written whenever
  /// any of it is.
  fn put(&mut self, new: Option<&[[u32; 4]]>, all: bool) -> Result<(), HandlerError> {
    use std::num::NonZeroU32;

    if !all && new.is_some_and(<[_]>::is_empty) {
      return Ok(());
    }

    let Some(Kept { size, row, pixels }) = &self.kept else {
      return Ok(());
    };
    let [Some(width), Some(height)] = size.map(NonZeroU32::new) else {
      return Ok(());
    };
    self.surface.resize(width, height)?;
    let mut shown = self.surface.buffer_mut()?;
    let whole = [[0, 0, size[0], size[1]]];
    write(
      &mut shown,
      size[0] as usize,
      &whole,
      0,
      pixels,
      *row as usize,
    );
    shown.present()?;
    Ok(())
  }
}

/// The window as softbuffer puts pixels in an X11 one.
///
/// softbuffer 0.4.8 keeps one surface for a window there, which holds the picture last put in
/// the window for as long as the window keeps its size (`X11Impl::resize` and `age` in its
/// `backends/x11.rs`). So the surface is the picture that is kept, and a frame is copied once,
/// from where it is read back into the surface.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl Screen {
  pub(super) fn new(context: &WindowContext) -> Result<Self, HandlerError> {
    let window = context.platform_handle();
    let display = softbuffer::Context::new(window.clone())?;
    Ok(Self {
      surface: softbuffer::Surface::new(&display, window)?,
      row: 0,
    })
  }

  /// Shows a picture over the whole window.
  ///
  /// `pixels` holds its rows from the top. Each row is `row` bytes long and starts with four
  /// bytes for every pixel across: blue, green, red and one more.
  pub(super) fn show(
    &mut self,
    size: [u32; 2],
    row: u32,
    pixels: &[u8],
  ) -> Result<(), HandlerError> {
    use std::num::NonZeroU32;

    assert!(row >= size[0] * 4, "a row holds the picture's width");
    let [Some(width), Some(height)] = size.map(NonZeroU32::new) else {
      return Ok(());
    };
    self.surface.resize(width, height)?;
    self.row = row;
    let mut shown = self.surface.buffer_mut()?;
    let whole = [[0, 0, size[0], size[1]]];
    write(
      &mut shown,
      size[0] as usize,
      &whole,
      0,
      pixels,
      row as usize,
    );
    shown.present()?;
    Ok(())
  }

  /// Shows a picture that is new in parts.
  ///
  /// The picture is of the size of the one shown last and differs from it in `parts`. Each part
  /// is `[left, top, width, height]` in the picture's pixels and lies on it. `pixels` holds the
  /// picture's rows from `top` on as [`Screen::show`] was given them, of which only the parts
  /// are read.
  ///
  /// `all` is whether the window may have lost any of the picture shown last. If it has not,
  /// the parts are all that is sent to it.
  ///
  /// Returns whether the picture is in the window. It is not if the surface no longer holds the
  /// picture shown last, and then the whole picture has to be shown.
  pub(super) fn patch(
    &mut self,
    parts: &[[u32; 4]],
    top: u32,
    pixels: &[u8],
    all: bool,
  ) -> Result<bool, HandlerError> {
    use std::num::NonZeroU32;

    if !all && parts.is_empty() {
      return Ok(true);
    }
    let mut shown = self.surface.buffer_mut()?;
    // at an age of one the surface holds the picture put in the window last
    if shown.age() != 1 {
      return Ok(false);
    }
    let wide = shown.width().get() as usize;
    write(&mut shown, wide, parts, top, pixels, self.row as usize);
    if all {
      shown.present()?;
      return Ok(true);
    }
    let sent: Vec<softbuffer::Rect> = (parts.iter())
      .filter_map(|&[x, y, width, height]| {
        let [Some(width), Some(height)] = [width, height].map(NonZeroU32::new) else {
          return None;
        };
        Some(softbuffer::Rect {
          x,
          y,
          width,
          height,
        })
      })
      .collect();
    shown.present_with_damage(&sent)?;
    Ok(true)
  }

  /// Puts the picture last shown in the window again.
  ///
  /// Returns whether the window has its picture. It has not if none was shown or the surface no
  /// longer holds it, and then the whole picture has to be shown.
  pub(super) fn again(&mut self) -> Result<bool, HandlerError> {
    self.patch(&[], 0, &[], true)
  }
}
