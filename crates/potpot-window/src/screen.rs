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
  kept: Option<Kept>,
}

/// A picture of `size` pixels.
///
/// `pixels` holds its rows from the top. Each row is `row` bytes long and starts with four
/// bytes for every pixel across: blue, green, red and one more.
pub(super) struct Kept {
  pub(super) size: [u32; 2],
  pub(super) row: u32,
  pub(super) pixels: Vec<u8>,
}

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
  /// # Panics
  ///
  /// Panics if no picture has been shown.
  pub(super) fn patch(
    &mut self,
    parts: &[[u32; 4]],
    top: u32,
    pixels: &[u8],
    all: bool,
  ) -> Result<(), HandlerError> {
    let kept = self.kept.as_mut().expect("a picture was shown before");
    kept.patch(parts, top, pixels);
    self.put(Some(parts), all)
  }

  /// Puts the picture last shown in the window again.
  ///
  /// With none shown yet it does nothing.
  pub(super) fn again(&mut self) -> Result<(), HandlerError> {
    self.put(Some(&[]), true)
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

  /// Puts the kept picture in the window.
  ///
  /// `new` is the parts of it that differ from what was put there last, or `None` for any part.
  /// The surface is written the parts alone where it still holds the rest. All of the picture
  /// is sent to the window if `all` is set, and else the parts alone.
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
    let (wide, row) = (size[0] as usize, *row as usize);
    let write = |shown: &mut [u32], [left, top, width, height]: [usize; 4]| {
      for down in top..top + height {
        let drawn = &pixels[down * row + left * 4..][..width * 4];
        let shown = &mut shown[down * wide + left..][..width];
        for (shown, &[blue, green, red, _]) in shown.iter_mut().zip(drawn.as_chunks::<4>().0) {
          // softbuffer has a pixel as a number with blue lowest. baseview's x11 window has a
          // 32 bit visual, where a compositor reads the byte above red as how opaque the pixel is
          *shown = u32::from_le_bytes([blue, green, red, u8::MAX]);
        }
      }
    };
    // at an age of one the surface holds the picture put there last, and at any other nothing
    // to go by
    let Some(new) = new.filter(|_| shown.age() == 1) else {
      write(&mut shown, [0, 0, wide, size[1] as usize]);
      shown.present()?;
      return Ok(());
    };
    let mut sent = Vec::with_capacity(new.len());
    for &[left, top, width, height] in new {
      let [Some(width), Some(height)] = [width, height].map(NonZeroU32::new) else {
        continue;
      };
      write(
        &mut shown,
        [left, top, width.get(), height.get()].map(|side| side as usize),
      );
      sent.push(softbuffer::Rect {
        x: left,
        y: top,
        width,
        height,
      });
    }
    if all {
      shown.present()?;
    } else {
      shown.present_with_damage(&sent)?;
    }
    Ok(())
  }
}
