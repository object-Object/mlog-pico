use embedded_graphics::pixelcolor::Rgb666;
use embedded_hal::delay::DelayNs;
use mipidsi::{
    dcs::{
        BitsPerPixel, EnterNormalMode, ExitSleepMode, InterfaceExt, PixelFormat, SetAddressMode,
        SetDisplayOn, SetInvertMode, SetPixelFormat,
    },
    interface::Interface,
    models::Model,
    options::ModelOptions,
};

// copied from mipidsi::models::ST7789 to change the color format
// https://github.com/almindor/mipidsi/blob/d85192a933623d6c069f22d5738e25c368f55808/src/models/st7789.rs

/// ST7789VW display in Rgb666 color mode.
pub struct ST7789VW;

impl Model for ST7789VW {
    type ColorFormat = Rgb666;
    const FRAMEBUFFER_SIZE: (u16, u16) = (240, 320);

    fn init<DELAY, DI>(
        &mut self,
        di: &mut DI,
        delay: &mut DELAY,
        options: &ModelOptions,
    ) -> Result<SetAddressMode, DI::Error>
    where
        DELAY: DelayNs,
        DI: Interface,
    {
        let madctl = SetAddressMode::from(options);

        delay.delay_us(150_000);

        di.write_command(ExitSleepMode)?;
        delay.delay_us(10_000);

        // set hw scroll area based on framebuffer size
        di.write_command(madctl)?;

        di.write_command(SetInvertMode::new(options.invert_colors))?;

        let pf = PixelFormat::with_all(BitsPerPixel::from_rgb_color::<Self::ColorFormat>());
        di.write_command(SetPixelFormat::new(pf))?;
        delay.delay_us(10_000);
        di.write_command(EnterNormalMode)?;
        delay.delay_us(10_000);
        di.write_command(SetDisplayOn)?;

        // DISPON requires some time otherwise we risk SPI data issues
        delay.delay_us(120_000);

        Ok(madctl)
    }
}
