use embassy_nrf::{
    Peri, gpio, interrupt,
    peripherals::{P0_05, P0_20, SAADC},
    saadc,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WakeWordError {
    #[error("Update would overflow ringbuffer")]
    WouldOverflow,
}

/// A RingBuffer that returns windows size `W` shifiting `S` elements ahead each
/// time.
///
/// ## Generic params
/// B is the size of the buffer, W is the size of the window, S is the amount of
/// the shift, and T is the underlying data type.
pub struct RingBuffer<const B: usize, const W: usize, const S: usize, T> {
    b: [T; B],
    next_write: usize,
    next_read: usize,
    free_space: usize,
}

impl<const B: usize, const W: usize, const S: usize, T: Copy> RingBuffer<B, W, S, T> {
    pub fn new(init: T) -> Self {
        let b = [init; B];
        let next_write = 0;
        let next_read = 0;
        let free_space = B;
        RingBuffer {
            b,
            next_write,
            next_read,
            free_space,
        }
    }

    /// This function writes the samples contained in `u` to the buffer.
    ///
    /// It returns an `Err(WakewordError::WouldOverflow)` and writes nothing if
    /// `u` is longer than the free spaces available in the buffer.
    pub fn update(&mut self, u: &[T]) -> Result<(), WakeWordError> {
        if u.len() > self.free_space {
            return Err(WakeWordError::WouldOverflow);
        }
        for i in 0..u.len() {
            self.b[(self.next_write + i) % B] = u[i];
        }
        self.next_write = (self.next_write + u.len()) % B;
        self.free_space -= u.len();
        Ok(())
    }

    /// This function returns a window of size `W` shifting `S` samples ahead at
    /// every call. If fewer than `W` unread samples are present in the buffer,
    /// returns None.
    pub fn frame(&mut self) -> Option<[T; W]> {
        let to_read = B - self.free_space;
        if to_read >= W {
            self.free_space += S;
            let buf = core::array::from_fn(|i| self.b[(self.next_read + i) % B]);
            self.next_read = (self.next_read + S) % B;
            Some(buf)
        } else {
            None
        }
    }
}

/// Enables the microphone via the enable pin and initializes the SAADC for wake
/// word recognition.
pub fn prepare_mic_saadc<'b>(
    saadc_peri: Peri<'b, SAADC>,
    p0_20_peri: Peri<'b, P0_20>,
    p0_05_peri: Peri<'b, P0_05>,
    irq: impl interrupt::typelevel::Binding<interrupt::typelevel::SAADC, saadc::InterruptHandler> + 'b,
) -> (saadc::Saadc<'b, 1>, gpio::Output<'b>) {
    let mic_pwr = gpio::Output::new(p0_20_peri, gpio::Level::High, gpio::OutputDrive::Standard);
    let mut config = saadc::Config::default();
    config.resolution = saadc::Resolution::_12BIT;
    let mut channel_config = saadc::ChannelConfig::single_ended(p0_05_peri);
    channel_config.gain = saadc::Gain::GAIN1_4;
    let saadc = saadc::Saadc::new(saadc_peri, irq, config, [channel_config]);
    (saadc, mic_pwr)
}

pub fn from_i16_pcm_to_f32(i: i16) -> f32 {
    i as f32 / 32768f32
}
