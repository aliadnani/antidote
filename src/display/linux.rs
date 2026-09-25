use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_8X13},
    pixelcolor::BinaryColor,
    prelude::Point,
    text::{Baseline, Text},
};
use embedded_hal_compat::ReverseCompat;
use linux_embedded_hal::{
    I2cdev, SpidevBus,
    spidev::{SpiModeFlags, SpidevOptions},
};
use sh1106::{
    Builder, displayrotation::DisplayRotation, interface::I2cInterface, mode::GraphicsMode,
};
use tracing::{error, warn};

use super::Display;

const I2C_DEVICE: &str = "/dev/i2c-7";
const I2C_ADDRESS: u8 = 0x3C;
const SPI_DEVICE: &str = "/dev/spidev1.0";
const SPI_SPEED_HZ: u32 = 2_400_000;
const ADDRESSABLE_LED_COUNT: usize = 4;
const PROFILE_BANK_COUNT: usize = 8;
const MAX_PROFILE_NAME_CHARS: usize = 16;

// FONT_8X13 is 8 pixels wide per character, so 16 characters fill the 128px display.
type Oled = GraphicsMode<I2cInterface<embedded_hal_compat::Reverse<I2cdev>>>;

pub struct PlatformDisplay {
    oled: Option<Oled>,
    leds: Option<SpidevBus>,
}

impl PlatformDisplay {
    pub fn new() -> Self {
        let oled = match I2cdev::new(I2C_DEVICE) {
            Ok(i2c) => {
                let mut oled: Oled = Builder::new()
                    .with_i2c_addr(I2C_ADDRESS)
                    .with_rotation(DisplayRotation::Rotate180)
                    .connect_i2c(i2c.reverse())
                    .into();
                if let Err(error) = oled.init() {
                    warn!(?error, "Failed to initialize SH1106 OLED");
                }
                Some(oled)
            }
            Err(error) => {
                warn!(
                    ?error,
                    device = I2C_DEVICE,
                    "Could not open OLED I2C device"
                );
                None
            }
        };

        let leds = match SpidevBus::open(SPI_DEVICE) {
            Ok(mut spi) => {
                let options = SpidevOptions::new()
                    .bits_per_word(8)
                    .max_speed_hz(SPI_SPEED_HZ)
                    .mode(SpiModeFlags::SPI_MODE_0)
                    .build();
                match spi.configure(&options) {
                    Ok(()) => Some(spi),
                    Err(error) => {
                        warn!(
                            ?error,
                            device = SPI_DEVICE,
                            "Could not configure addressable LED SPI device"
                        );
                        None
                    }
                }
            }
            Err(error) => {
                warn!(
                    ?error,
                    device = SPI_DEVICE,
                    "Could not open addressable LED SPI device"
                );
                None
            }
        };

        Self { oled, leds }
    }
}

impl Display for PlatformDisplay {
    fn render(&mut self, profile_name: &str, profile_index: usize) {
        if let Some(oled) = &mut self.oled {
            let profile_name = truncate_profile_name(profile_name);
            let text_style = MonoTextStyle::new(&FONT_8X13, BinaryColor::On);

            oled.clear();
            let text_width = profile_name.chars().count() as i32 * 8;
            let x = (128 - text_width) / 2;
            let _ =
                Text::with_baseline(&profile_name, Point::new(x, 25), text_style, Baseline::Top)
                    .draw(&mut *oled);

            if let Err(error) = oled.flush() {
                warn!(?error, "Failed to render profile name on SH1106 OLED");
            }
        }

        if let Some(leds) = &mut self.leds {
            let data = encode_led_frame(profile_index);
            if let Err(error) = embedded_hal_compat::eh1_0::spi::SpiBus::write(leds, &data)
                .and_then(|()| embedded_hal_compat::eh1_0::spi::SpiBus::flush(leds))
            {
                error!(?error, "Failed to update addressable LEDs");
            }
        }
    }
}

fn truncate_profile_name(profile_name: &str) -> String {
    let mut chars = profile_name.chars();
    let truncated: String = chars.by_ref().take(MAX_PROFILE_NAME_CHARS).collect();
    if chars.next().is_some() {
        format!(
            "{}...",
            truncated
                .chars()
                .take(MAX_PROFILE_NAME_CHARS - 3)
                .collect::<String>()
        )
    } else {
        truncated
    }
}

fn encode_led_frame(profile_index: usize) -> [u8; 8 + ADDRESSABLE_LED_COUNT * 9] {
    let mut frame = [0; 8 + ADDRESSABLE_LED_COUNT * 9];
    let profile_slot = profile_index % PROFILE_BANK_COUNT;
    let led_index = profile_slot % ADDRESSABLE_LED_COUNT;
    let color = if profile_slot < ADDRESSABLE_LED_COUNT {
        [0, 255, 0] // Green for profile slots 1-4.
    } else {
        [255, 255, 0] // Yellow for profile slots 5-8.
    };

    for (index, led_color) in (0..ADDRESSABLE_LED_COUNT)
        .map(|index| if index == led_index { color } else { [0, 0, 0] })
        .enumerate()
    {
        for (channel_index, channel) in [led_color[1], led_color[0], led_color[2]]
            .into_iter()
            .enumerate()
        {
            let encoded = encode_byte(channel);
            let offset = 8 + index * 9 + channel_index * 3;
            frame[offset..offset + 3].copy_from_slice(&encoded);
        }
    }

    frame
}

fn encode_byte(byte: u8) -> [u8; 3] {
    const BIT_1: u32 = 0b110;
    const BIT_0: u32 = 0b100;

    let mut encoded = 0;
    for bit_index in (0..8).rev() {
        let bit = (byte >> bit_index) & 1;
        encoded = (encoded << 3) | if bit == 1 { BIT_1 } else { BIT_0 };
    }

    [
        ((encoded >> 16) & 0xff) as u8,
        ((encoded >> 8) & 0xff) as u8,
        (encoded & 0xff) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::{ADDRESSABLE_LED_COUNT, encode_led_frame, truncate_profile_name};

    #[test]
    fn profile_led_selection_wraps_after_eight_profiles() {
        assert_eq!(encode_led_frame(0), encode_led_frame(8));
        assert_eq!(encode_led_frame(4), encode_led_frame(12));
        assert_ne!(encode_led_frame(0), encode_led_frame(4));
        assert_eq!(ADDRESSABLE_LED_COUNT, 4);
    }

    #[test]
    fn profile_filename_is_limited_to_sixteen_characters() {
        assert_eq!(
            truncate_profile_name("fender_brown.nam"),
            "fender_brown.nam"
        );
        assert_eq!(
            truncate_profile_name("a_long_profile_filename.nam"),
            "a_long_profi..."
        );
        assert_eq!(truncate_profile_name("guitar_é.nam"), "guitar_é.nam");
    }
}
