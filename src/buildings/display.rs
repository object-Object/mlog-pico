use core::fmt::Debug;

use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle},
    text::{Alignment, Baseline, LineHeight, Text, TextStyleBuilder},
};
use mindy::{
    types::LAccess,
    vm::{
        CustomBuildingData, DrawCommand, InstructionResult, LValue, LogicVM, ProcessorState,
        TextAlignment,
    },
};

include!(concat!(env!("OUT_DIR"), "/logic.rs"));

// https://github.com/Anuken/Mindustry/blob/65a50a97423431640e636463dde97f6f88a2b0c8/core/src/mindustry/graphics/Pal.java#L35
pub const DISPLAY_RESET_COLOR: Rgb888 = Rgb888::new(0x56, 0x56, 0x66);

pub struct DisplayData<T>
where
    T: DrawTarget,
{
    display: T,
    size: Size,
    line_style: PrimitiveStyle<T::Color>,
    fill_style: PrimitiveStyle<T::Color>,
    char_style: MonoTextStyle<'static, T::Color>,
    translation: Point,
    operations: usize,
}

impl<T> DisplayData<T>
where
    T: DrawTarget,
    T::Color: From<Rgb888>,
{
    pub fn new(display: T) -> Self {
        let color = Rgb888::WHITE.into();

        Self {
            size: display.bounding_box().size,
            display,
            line_style: PrimitiveStyleBuilder::new()
                .stroke_color(color)
                .stroke_width(1)
                .build(),
            fill_style: PrimitiveStyleBuilder::new().fill_color(color).build(),
            char_style: MonoTextStyle::new(&LOGIC, color),
            translation: Point::zero(),
            operations: 0,
        }
    }

    fn point(&self, x: i16, y: i16) -> Point {
        let mut point = Point::new(x as i32, y as i32);

        point += self.translation;

        // mindustry displays start at 1, not 0
        point -= Point::new(1, 1);

        // invert y
        Point {
            x: point.x,
            y: self.size.height as i32 - point.y - 1,
        }
    }

    fn draw_command(&mut self, command: &DrawCommand) -> Result<(), T::Error> {
        match command {
            &DrawCommand::Clear { r, g, b } => self.display.clear(Rgb888::new(r, g, b).into()),

            &DrawCommand::Color { r, g, b, a } => {
                let color = if a > 0 {
                    Some(Rgb888::new(r, g, b).into())
                } else {
                    None
                };
                self.line_style.stroke_color = color;
                self.fill_style.fill_color = color;
                self.char_style.text_color = color;
                Ok(())
            }

            &DrawCommand::Stroke { width } => {
                self.line_style.stroke_width = width as u32;
                Ok(())
            }

            &DrawCommand::Line { x1, y1, x2, y2 } => {
                Line::new(self.point(x1, y1), self.point(x2, y2))
                    .into_styled(self.line_style)
                    .draw(&mut self.display)
            }

            &DrawCommand::Rect {
                x,
                y,
                width,
                height,
                fill,
            } => Rectangle::new(
                // add height to get the top left corner instead of bottom left
                self.point(x, y + height)
                // i don't know why mindustry offsets rects like this
                + Point::new(1, 0),
                Size::new(width as u32, height as u32),
            )
            .into_styled(if fill {
                self.fill_style
            } else {
                self.line_style
            })
            .draw(&mut self.display),

            // TODO: implement
            // fill: https://github.com/embedded-graphics/embedded-graphics/issues/293
            &DrawCommand::Poly { .. } => Ok(()),

            &DrawCommand::Triangle {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
            } => Triangle::new(self.point(x1, y1), self.point(x2, y2), self.point(x3, y3))
                .into_styled(self.fill_style)
                .draw(&mut self.display),

            // TODO: implement
            &DrawCommand::Image { .. } => Ok(()),

            DrawCommand::Print {
                x,
                y,
                alignment,
                text,
            } => {
                let mut position = self.point(*x + 1, *y - 2);

                let style_alignment = if alignment.contains(TextAlignment::LEFT) {
                    Alignment::Left
                } else if alignment.contains(TextAlignment::RIGHT) {
                    position.x -= 1; // ??????
                    Alignment::Right
                } else {
                    Alignment::Center
                };

                let baseline = if alignment.contains(TextAlignment::BOTTOM) {
                    Baseline::Bottom
                } else if alignment.contains(TextAlignment::TOP) {
                    position.y += 1;
                    Baseline::Top
                } else {
                    Baseline::Middle
                };

                let text_style = TextStyleBuilder::new()
                    .alignment(style_alignment)
                    .baseline(baseline)
                    .line_height(LineHeight::Pixels(13))
                    .build();

                Text {
                    text: &text.to_string_lossy(),
                    position,
                    character_style: self.char_style,
                    text_style,
                }
                .draw(&mut self.display)
                .map(|_| ())
            }

            &DrawCommand::Translate { x, y } => {
                self.translation += Point::new(x as i32, y as i32);
                Ok(())
            }

            // TODO: implement
            DrawCommand::Scale { .. } | DrawCommand::Rotate { .. } => Ok(()),

            DrawCommand::Reset => {
                self.translation = Point::zero();
                Ok(())
            }
        }
    }
}

impl<T> CustomBuildingData for DisplayData<T>
where
    T: DrawTarget,
    T::Color: From<Rgb888>,
    T::Error: Debug,
{
    fn drawflush(&mut self, state: &mut ProcessorState, _: &LogicVM) -> InstructionResult {
        for command in &state.drawbuffer {
            self.draw_command(command).unwrap();
        }

        self.operations += 1;

        // we just did a lot of blocking calls, so yield to let other threads run
        // TODO: use async instead?
        InstructionResult::Yield
    }

    fn sensor(&mut self, _: &mut ProcessorState, _: &LogicVM, sensor: LAccess) -> Option<LValue> {
        Some(match sensor {
            LAccess::DisplayWidth => self.size.width.into(),
            LAccess::DisplayHeight => self.size.height.into(),
            LAccess::Operations => self.operations.into(),
            _ => return None,
        })
    }
}
